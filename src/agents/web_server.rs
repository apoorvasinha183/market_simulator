// src/agents/web_server.rs

use crate::{
    agents::web_proxy_agent::ProxyRequest, simulation::candle_analyzer::CandleDataHandle,
    simulation::orchestra::ShadowBookHandle,
    simulation::price_history_tracker::PriceHistoryTracker, simulators::order_book::OrderBook,
    types::candle::TimeFrame, types::order::Side,
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
        println!("[WebServerRunner] Starting web server with price history tracking...");

        // Create price history tracker (keep 10,000 price points per stock)
        let price_history_handle = Arc::new(PriceHistoryTracker::new(10_000, 200));
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
                    price_history_handle: price_history_handle.clone(),
                    proxy_request_tx,
                });

                tokio::spawn(run_broadcast_loop(
                    view_handle,
                    candle_handle,
                    price_history_handle,
                    broadcast_tx,
                ));

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
    price_history_handle: Arc<PriceHistoryTracker>,
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
    RequestSnapshot { context: Option<SnapshotContext> },
    ChangeContext(ContextChangePayload),
}

#[derive(Deserialize, Debug)]
struct ContextChangePayload {
    page: Option<String>,
    selected_stocks: Option<Vec<u64>>,
    timeframe: Option<String>,
}

#[derive(Deserialize, Debug)]
struct SnapshotContext {
    page: Option<String>,
    selected_stocks: Option<Vec<u64>>,
    timeframe: Option<String>,
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
    let (snapshot_request_tx, mut snapshot_request_rx) =
        tokio::sync::mpsc::unbounded_channel::<Option<SnapshotContext>>();
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

    // Use the proper price history tracker instead of deriving from candles
    let initial_price_history = state.price_history_handle.get_all_price_histories();

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

    // Convert Arc<OrderBook> to OrderBook for serialization
    let order_books_for_serialization: HashMap<u64, &OrderBook> = initial_market_state
        .order_books
        .iter()
        .map(|(k, v)| (*k, v.as_ref()))
        .collect();

    let initial_snapshot = json!({
        "type": "snapshot",
        "data": {
            "market_state": {
                "order_books": order_books_for_serialization,
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
    let state_clone = state.clone();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                // Forward market data broadcasts to the client
                Ok(msg) = broadcast_rx.recv() => {
                    if sender.send(Message::Text(msg)).await.is_err() {
                        break;
                    }
                },
                // Handle snapshot requests
                context = snapshot_request_rx.recv() => {
                    let context = match context {
                        Some(ctx) => ctx,
                        None => break,
                    };
                    if let Ok(snapshot) = generate_snapshot(&state_clone, context.as_ref()).await {
                        if sender.send(Message::Text(snapshot)).await.is_err() {
                            break;
                        }
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
                    ClientMessage::RequestSnapshot { context } => {
                        println!(
                            "[WebServer] Client requested snapshot with context: {:?}",
                            context
                        );
                        // Send snapshot request to the spawned task
                        if snapshot_request_tx.send(context).is_err() {
                            break;
                        }
                    }
                    ClientMessage::ChangeContext(payload) => {
                        println!(
                            "[WebServer] Client changed context: page={:?}, stocks={:?}, timeframe={:?}",
                            payload.page, payload.selected_stocks, payload.timeframe
                        );

                        // Convert payload to SnapshotContext and request fresh snapshot
                        let context = SnapshotContext {
                            page: payload.page.clone(),
                            selected_stocks: payload.selected_stocks.clone(),
                            timeframe: payload.timeframe.clone(),
                        };

                        if snapshot_request_tx.send(Some(context)).is_err() {
                            break;
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

async fn generate_snapshot(
    state: &AppState,
    context: Option<&SnapshotContext>,
) -> Result<String, String> {
    let market_state = state.view_handle.read().unwrap().clone();

    // Filter data based on context if provided
    let (candle_data, price_history) = if let Some(ctx) = context {
        // Filter by selected stocks if specified
        let stock_filter = ctx.selected_stocks.as_ref();

        let filtered_candles = state
            .candle_handle
            .iter()
            .filter(|entry| {
                let (stock_id, _) = *entry.key();
                stock_filter.map_or(true, |stocks| stocks.contains(&stock_id))
            })
            .map(|entry| {
                let (stock_id, timeframe) = *entry.key();
                let key = format!("{}-{}", stock_id, timeframe);
                (key, entry.value().iter().cloned().collect::<Vec<_>>())
            })
            .collect::<HashMap<String, Vec<_>>>();

        let filtered_price_history = if let Some(stocks) = stock_filter {
            let all_histories = state.price_history_handle.get_all_price_histories();
            stocks
                .iter()
                .filter_map(|&stock_id| {
                    all_histories
                        .get(&stock_id)
                        .map(|history| (stock_id, history.clone()))
                })
                .collect::<HashMap<u64, Vec<[f64; 2]>>>()
        } else {
            state.price_history_handle.get_all_price_histories()
        };

        (filtered_candles, filtered_price_history)
    } else {
        // No context filtering - send everything
        let all_candles = state
            .candle_handle
            .iter()
            .map(|entry| {
                let (stock_id, timeframe) = *entry.key();
                let key = format!("{}-{}", stock_id, timeframe);
                (key, entry.value().iter().cloned().collect::<Vec<_>>())
            })
            .collect::<HashMap<String, Vec<_>>>();

        let all_price_history = state.price_history_handle.get_all_price_histories();
        (all_candles, all_price_history)
    };

    let mut mid_prices = HashMap::new();
    let mut spreads = HashMap::new();
    for stock in market_state.stocks.get_all_stocks() {
        if let Some(mid) = market_state.get_mid_price(stock.id) {
            mid_prices.insert(stock.id, mid as f64 / 100.0);
        }
        if let Some(spread) = market_state.get_spread(stock.id) {
            spreads.insert(stock.id, spread as f64 / 100.0);
        }
    }

    // Convert Arc<OrderBook> to OrderBook for serialization
    let order_books_for_serialization: HashMap<u64, &OrderBook> = market_state
        .order_books
        .iter()
        .map(|(k, v)| (*k, v.as_ref()))
        .collect();

    let snapshot = json!({
        "type": "snapshot",
        "data": {
            "market_state": {
                "order_books": order_books_for_serialization,
                "stocks": market_state.stocks,
                "last_traded_price": market_state.last_traded_price,
                "cumulative_volume": market_state.cumulative_volume,
                "mid_prices": mid_prices,
                "spreads": spreads,
            },
            "candle_data": candle_data,
            "price_history": price_history,
        }
    });

    serde_json::to_string(&snapshot).map_err(|e| e.to_string())
}

// Updated broadcast loop with price history tracking
async fn run_broadcast_loop(
    view_handle: ShadowBookHandle,
    candle_handle: CandleDataHandle,
    price_history_handle: Arc<PriceHistoryTracker>,
    tx: broadcast::Sender<String>,
) {
    let mut price_interval = tokio::time::interval(Duration::from_millis(200)); // Slower price updates
    let mut sent_candles_count: HashMap<String, usize> = HashMap::new();

    loop {
        // Wait for the shorter interval (50ms for price updates)
        price_interval.tick().await;

        if tx.receiver_count() == 0 {
            continue;
        }

        // Quick read for price data and record mid prices for history
        let (last_traded_price, mid_prices, spreads, cumulative_volume) = {
            let state = view_handle.read().unwrap();
            let mut mid_prices = HashMap::new();
            let mut spreads = HashMap::new();

            for stock in state.stocks.get_all_stocks() {
                if let Some(mid) = state.get_mid_price(stock.id) {
                    let mid_price_dollars = mid as f64 / 100.0;
                    mid_prices.insert(stock.id, mid_price_dollars);

                    // Record this mid price in our price history tracker
                    price_history_handle.update_price(stock.id, mid);
                }
                if let Some(spread) = state.get_spread(stock.id) {
                    spreads.insert(stock.id, spread as f64 / 100.0);
                }
            }

            (
                state.last_traded_price.clone(),
                mid_prices,
                spreads,
                state.cumulative_volume.clone(),
            )
        };

        // Send lightweight price update
        let price_update = json!({
            "type": "price_update",
            "data": {
                "last_traded_price": last_traded_price,
                "mid_prices": mid_prices,
                "spreads": spreads,
                "cumulative_volume": cumulative_volume,
            }
        });

        let price_msg = serde_json::to_string(&price_update).unwrap();
        let _ = tx.send(price_msg);

        // Check for new candles and send if available
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

        if !new_candles.is_empty() {
            let candle_update = json!({
                "type": "candle_update",
                "data": {
                    "candle_data": new_candles
                }
            });
            let candle_msg = serde_json::to_string(&candle_update).unwrap();
            let _ = tx.send(candle_msg);
        }

        // Check if it's time for order book and price history updates
        static mut UPDATE_COUNTER: u32 = 0;
        static mut PRICE_HISTORY_COUNTER: u32 = 0;
        static mut LAST_PRICE_HISTORY_TIMESTAMP: u64 = 0;

        unsafe {
            UPDATE_COUNTER += 1;
            PRICE_HISTORY_COUNTER += 1;

            // Send price history updates every 10 intervals (2 seconds) - much less frequent for line charts
            if PRICE_HISTORY_COUNTER >= 10 {
                PRICE_HISTORY_COUNTER = 0;

                let current_timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;

                let price_history_updates = price_history_handle
                    .get_price_updates_since(LAST_PRICE_HISTORY_TIMESTAMP as f64 / 1000.0);

                if !price_history_updates.is_empty() {
                    let price_history_update = json!({
                        "type": "price_history_update",
                        "data": {
                            "price_history": price_history_updates
                        }
                    });

                    let price_history_msg = serde_json::to_string(&price_history_update).unwrap();
                    let _ = tx.send(price_history_msg);
                }

                LAST_PRICE_HISTORY_TIMESTAMP = current_timestamp;
            }

            // Send order book updates less frequently (every 5 intervals = 1 second)
            if UPDATE_COUNTER >= 5 {
                UPDATE_COUNTER = 0;

                // Read order books less frequently
                let order_books_arc = {
                    let state = view_handle.read().unwrap();
                    state.order_books.clone()
                };

                // Convert Arc<OrderBook> to OrderBook for serialization
                let order_books_for_serialization: HashMap<u64, &OrderBook> = order_books_arc
                    .iter()
                    .map(|(k, v)| (*k, v.as_ref()))
                    .collect();

                let orderbook_update = json!({
                    "type": "orderbook_update",
                    "data": {
                        "order_books": order_books_for_serialization
                    }
                });

                let orderbook_msg = serde_json::to_string(&orderbook_update).unwrap();
                let _ = tx.send(orderbook_msg);
            }
        }
    }
}
