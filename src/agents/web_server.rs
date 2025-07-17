// src/agents/web_server.rs

use crate::{
    agents::web_proxy_agent::ProxyRequest, simulation::candle_analyzer::CandleDataHandle,
    simulation::orchestra::ShadowBookHandle, types::candle::TimeFrame, types::order::Side,
};
use axum::{
    Router,
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
    routing::get,
};
use crossbeam_channel::{Sender, unbounded};
use futures_util::{sink::SinkExt, stream::StreamExt};
use serde::Deserialize;
use serde_json::json;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::broadcast;

pub struct WebServerRunner;

impl WebServerRunner {
    pub fn run(
        view_handle: ShadowBookHandle,
        candle_handle: CandleDataHandle,
        proxy_request_tx: Sender<ProxyRequest>,
    ) {
        println!("[WebServerRunner] Starting web server...");

        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let (broadcast_tx, _) = broadcast::channel(1024);

                let app_state = Arc::new(AppState {
                    broadcast_tx: broadcast_tx.clone(),
                    view_handle: view_handle.clone(),
                    candle_handle: candle_handle.clone(),
                    proxy_request_tx,
                });

                tokio::spawn(run_broadcast_loop(view_handle, candle_handle, broadcast_tx));

                let router = Router::new()
                    .route("/ws", get(websocket_handler))
                    .with_state(app_state);

                let listener = tokio::net::TcpListener::bind("127.0.0.1:6969")
                    .await
                    .unwrap();
                println!(
                    "[WebServerRunner] WebSocket server listening on {}",
                    listener.local_addr().unwrap()
                );
                axum::serve(listener, router).await.unwrap();
            });
    }
}

struct AppState {
    broadcast_tx: broadcast::Sender<String>,
    view_handle: ShadowBookHandle,
    candle_handle: CandleDataHandle,
    proxy_request_tx: Sender<ProxyRequest>,
}

async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| websocket_connection(socket, state))
}

#[derive(Deserialize)]
#[serde(tag = "type", content = "payload")]
#[serde(rename_all = "PascalCase")]
enum ClientMessage {
    Register { client_id: String },
    SubmitOrder(SubmitOrderPayload),
}

#[derive(Deserialize)]
struct SubmitOrderPayload {
    stock_id: u64,
    side: Side,
    order_type: String,
    volume: u64,
    price: Option<f64>,
}

async fn websocket_connection(stream: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = stream.split();
    let (client_response_tx, client_response_rx) = unbounded();
    let mut broadcast_rx = state.broadcast_tx.subscribe();
    let client_uuid = Arc::new(tokio::sync::Mutex::new(None));

    // --- Send initial snapshot ---
    let initial_market_state = state.view_handle.read().unwrap().clone();
    let initial_candle_data = state
        .candle_handle
        .iter()
        .map(|entry| {
            let (stock_id, timeframe) = *entry.key();
            let key = format!("{}-{}", stock_id, timeframe);
            (key, entry.value().iter().cloned().collect::<Vec<_>>()) // Convert VecDeque to Vec
        })
        .collect::<HashMap<String, Vec<_>>>();

    let mut initial_price_history = HashMap::new();
    for (key, candles) in &initial_candle_data {
        if let Some(stock_id_str) = key.split('-').next() {
            if let Ok(stock_id) = stock_id_str.parse::<u64>() {
                let history = initial_price_history
                    .entry(stock_id)
                    .or_insert_with(Vec::new);
                for candle in candles {
                    history.push([candle.timestamp as f64, candle.close]);
                }
                history
                    .sort_by(|a, b| a[0].partial_cmp(&b[0]).unwrap_or(std::cmp::Ordering::Equal));
            }
        }
    }

    let mut initial_mid_prices = HashMap::new();
    let mut initial_spreads = HashMap::new();
    for stock in initial_market_state.stocks.get_all_stocks() {
        if let Some(mid) = initial_market_state.get_mid_price(stock.id) {
            initial_mid_prices.insert(stock.id, mid as f64 / 100.0);
        }
        if let Some(spread) = initial_market_state.get_spread(stock.id) {
            initial_spreads.insert(stock.id, spread as f64 / 100.0);
        }
    }

    let initial_snapshot = json!({
        "type": "snapshot",
        "data": {
            "market_state": {
                "order_books": initial_market_state.order_books,
                "stocks": initial_market_state.stocks,
                "last_traded_price": initial_market_state.last_traded_price,
                "cumulative_volume": initial_market_state.cumulative_volume,
                "mid_prices": initial_mid_prices,
                "spreads": initial_spreads,
            },
            "candle_data": initial_candle_data,
            "price_history": initial_price_history,
        }
    });

    let snapshot_msg = serde_json::to_string(&initial_snapshot).unwrap();
    if sender.send(Message::Text(snapshot_msg)).await.is_err() {
        eprintln!("[WebServer] Failed to send initial snapshot to client.");
        return; // Client likely disconnected immediately
    }

    // Spawn a task to handle messages from the proxy agent and market data broadcasts
    tokio::spawn(async move {
        loop {
            tokio::select! {
                // Forward market data broadcasts to the client
                Ok(msg) = broadcast_rx.recv() => {
                    if sender.send(Message::Text(msg)).await.is_err() {
                        break;
                    }
                },
                // Non-blocking check for proxy agent responses
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(10)) => {
                    if let Ok(response) = client_response_rx.try_recv() {
                        let msg = serde_json::to_string(&response).unwrap();
                        //println!("[WebServer] Sending ClientResponse: {}", msg);
                        if sender.send(Message::Text(msg)).await.is_err() {
                            break;
                        }
                    }
                }
            }
        }
    });

    // Handle incoming messages from the client
    while let Some(Ok(msg)) = receiver.next().await {
        if let Message::Text(text) = msg {
            match serde_json::from_str::<ClientMessage>(&text) {
                Ok(client_msg) => match client_msg {
                    ClientMessage::Register { client_id } => {
                        println!("[WebServer] Registering client: {}", client_id);
                        *client_uuid.lock().await = Some(client_id.clone());
                        let req = ProxyRequest::Register {
                            client_uuid: client_id,
                            response_tx: client_response_tx.clone(),
                        };
                        if let Err(e) = state.proxy_request_tx.send(req) {
                            eprintln!(
                                "[WebServer] Failed to send register request to proxy agent: {}",
                                e
                            );
                        }
                    }
                    ClientMessage::SubmitOrder(payload) => {
                        if let Some(uuid) = &*client_uuid.lock().await {
                            let req = ProxyRequest::SubmitOrder {
                                client_uuid: uuid.clone(),
                                stock_id: payload.stock_id,
                                side: payload.side,
                                order_type: payload.order_type,
                                volume: payload.volume,
                                price: payload.price,
                            };
                            if let Err(e) = state.proxy_request_tx.send(req) {
                                eprintln!(
                                    "[WebServer] Failed to send submit order request to proxy agent: {}",
                                    e
                                );
                            }
                        } else {
                            eprintln!("[WebServer] Received order before client was registered.");
                        }
                    }
                },
                Err(e) => {
                    eprintln!(
                        "[WebServer] Failed to parse client message: {}. Raw: '{}'",
                        e, text
                    );
                }
            }
        } else if matches!(msg, Message::Close(_)) {
            break;
        }
    }
}

// This function remains largely the same, broadcasting general market data
async fn run_broadcast_loop(
    view_handle: ShadowBookHandle,
    candle_handle: CandleDataHandle,
    tx: broadcast::Sender<String>,
) {
    let mut interval = tokio::time::interval(Duration::from_millis(1));
    let mut sent_candles_count: HashMap<String, usize> = HashMap::new();

    loop {
        interval.tick().await;
        if tx.receiver_count() == 0 {
            continue;
        }

        // Minimize lock time by reading state quickly
        let market_state = {
            let state = view_handle.read().unwrap();
            state.clone()
        };
        let stocks_for_calc = market_state.stocks.get_all_stocks();
        
        let new_candles: HashMap<String, Vec<_>> = candle_handle
            .iter()
            .filter(|entry| {
                let (_, timeframe) = entry.key();
                !matches!(timeframe, &TimeFrame::HundredMillis | &TimeFrame::OneSecond)
            })
            .filter_map(|entry| {
                let (stock_id, timeframe) = entry.key();
                let key = format!("{}-{}", stock_id, timeframe);
                let candles = entry.value();
                let current_count = candles.len();
                let last_count = sent_candles_count.entry(key.clone()).or_insert(0);

                if current_count > *last_count {
                    let new_slice = candles
                        .iter()
                        .skip(*last_count)
                        .cloned()
                        .collect::<Vec<_>>();
                    *last_count = current_count;
                    Some((key, new_slice))
                } else {
                    None
                }
            })
            .collect();

        let mut mid_prices = HashMap::new();
        let mut spreads = HashMap::new();
        for stock in stocks_for_calc {
            if let Some(mid) = market_state.get_mid_price(stock.id) {
                mid_prices.insert(stock.id, mid as f64 / 100.0);
            }
            if let Some(spread) = market_state.get_spread(stock.id) {
                spreads.insert(stock.id, spread as f64 / 100.0);
            }
        }

        let update_message = json!({
            "type": "update",
            "data": {
                "market_state": {
                    "order_books": market_state.order_books,
                    "stocks": market_state.stocks,
                    "last_traded_price": market_state.last_traded_price,
                    "cumulative_volume": market_state.cumulative_volume,
                    "mid_prices": mid_prices,
                    "spreads": spreads,
                },
                "candle_data": new_candles
            }
        });

        if !update_message["data"].is_null() {
            let msg = serde_json::to_string(&update_message).unwrap();
            let _ = tx.send(msg);
        }
    }
}
