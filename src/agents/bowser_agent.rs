// src/agents/bowser_agent.rs

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::{
    Router,
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
    routing::get,
};

use crossbeam_channel::{Receiver, Sender};
use futures_util::{sink::SinkExt, stream::StreamExt};
use serde_json::json;
use tokio::sync::broadcast;

use crate::{
    agents::agent_trait::Agent,
    simulation::{
        candle_analyzer::CandleDataHandle,
        orchestra::{MarketState, ShadowBookHandle},
    },
    types::order::{Order, OrderRequest, Trade},
};

#[derive(Clone)]
pub struct BowserAgent {
    id: usize,
    _order_channel: Sender<OrderRequest>,
    _ack_channel: Arc<Mutex<Receiver<Order>>>,
    _port_channel: Arc<Mutex<Receiver<Trade>>>,
    view_handle: ShadowBookHandle,
    candle_handle: CandleDataHandle,
}

impl BowserAgent {
    pub fn new(
        id: usize,
        order_channel: Sender<OrderRequest>,
        ack_channel: Receiver<Order>,
        port_channel: Receiver<Trade>,
        view_handle: ShadowBookHandle,
        candle_handle: CandleDataHandle,
    ) -> Self {
        Self {
            id,
            _order_channel: order_channel,
            _ack_channel: Arc::new(Mutex::new(ack_channel)),
            _port_channel: Arc::new(Mutex::new(port_channel)),
            view_handle,
            candle_handle,
        }
    }
}

impl Agent for BowserAgent {
    fn run(&mut self) {
        println!("[BowserAgent {}] Starting web server...", self.id);
        let view_handle = self.view_handle.clone();
        let candle_handle = self.candle_handle.clone();
        let agent_id = self.id;

        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let (tx, _) = broadcast::channel(1024);

                let app_state = Arc::new(AppState {
                    tx: tx.clone(),
                    view_handle: view_handle.clone(),
                    candle_handle: candle_handle.clone(),
                });

                // Task to periodically gather and broadcast incremental updates
                tokio::spawn(async move {
                    let mut interval = tokio::time::interval(Duration::from_millis(250));
                    let mut sent_candles_count: HashMap<String, usize> = HashMap::new();

                    loop {
                        interval.tick().await;
                        let market_state = view_handle.read().unwrap().clone();

                        let new_candles: HashMap<String, Vec<_>> = candle_handle
                            .iter()
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
                        for stock in market_state.stocks.get_all_stocks() {
                            if let Some(mid) = market_state.get_mid_price(stock.id) {
                                mid_prices.insert(stock.id.to_string(), mid as f64 / 100.0);
                            }
                            if let Some(spread) = market_state.get_spread(stock.id) {
                                spreads.insert(stock.id.to_string(), spread as f64 / 100.0);
                            }
                        }

                        let mut data_payload = serde_json::Map::new();
                        data_payload.insert("stocks".to_string(), json!(market_state.stocks));
                        data_payload.insert(
                            "last_traded_price".to_string(),
                            json!(market_state.last_traded_price),
                        );
                        data_payload.insert(
                            "cumulative_volume".to_string(),
                            json!(market_state.cumulative_volume),
                        );
                        data_payload
                            .insert("order_books".to_string(), json!(market_state.order_books));
                        data_payload.insert("mid_prices".to_string(), json!(mid_prices));
                        data_payload.insert("spreads".to_string(), json!(spreads));

                        if !new_candles.is_empty() {
                            data_payload.insert("candle_data".to_string(), json!(new_candles));
                        }

                        let update_message = json!({
                            "type": "update",
                            "data": {
                                "market_state": data_payload
                            }
                        });

                        if tx.receiver_count() > 0 {
                            let msg = serde_json::to_string(&update_message).unwrap();
                            if tx.send(msg).is_err() {
                                // In case of a send error, we can log it if needed.
                            }
                        }
                    }
                });

                let router = Router::new()
                    .route("/ws", get(websocket_handler))
                    .with_state(app_state);

                let listener = tokio::net::TcpListener::bind("127.0.0.1:6969")
                    .await
                    .unwrap();
                println!(
                    "[BowserAgent {}] WebSocket server listening on {}",
                    agent_id,
                    listener.local_addr().unwrap()
                );
                axum::serve(listener, router).await.unwrap();
            });
    }

    // --- Other required trait methods (mostly stubs for this agent) ---
    fn decide_actions(&mut self) {}
    fn buy_stock(&mut self, _stock_id: u64, _volume: u64) {}
    fn sell_stock(&mut self, _stock_id: u64, _volume: u64) {}
    fn acknowledge_order(&mut self) {}
    fn margin_call(&mut self) {}
    fn update_portfolio(&mut self) {}
    fn evaluate_port(&mut self, _market_view: &MarketState) -> f64 {
        0.0
    }
    fn get_pending_orders(&self) -> Vec<Order> {
        vec![]
    }
    fn cancel_open_order(&mut self, _order_id: u64) {}
    fn get_id(&self) -> usize {
        self.id
    }
    fn get_inventory(&self) -> i64 {
        0
    }
    fn clone_agent(&self) -> Box<dyn Agent> {
        Box::new(self.clone())
    }
}

struct AppState {
    tx: broadcast::Sender<String>,
    view_handle: ShadowBookHandle,
    candle_handle: CandleDataHandle,
}

use crate::types::candle::Candle;
use serde::Deserialize;

async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| websocket(socket, state))
}

#[derive(Deserialize)]
struct ClientMessage {
    r#type: String,
    stock_id: Option<u64>,
}

// ... (rest of the file is the same until the websocket function)

async fn websocket(stream: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = stream.split();

    // Send initial comprehensive snapshot
    let market_state = state.view_handle.read().unwrap().clone();

    let candle_history: HashMap<String, Vec<Candle>> = state
        .candle_handle
        .iter()
        .map(|entry| {
            let (id, timeframe) = entry.key();
            (
                format!("{}-{}", id, timeframe),
                entry.value().clone().into_iter().collect(),
            )
        })
        .collect();

    let mut price_history: HashMap<u64, Vec<(i64, f64)>> = HashMap::new();
    for (key, candles) in &candle_history {
        if let Some(stock_id_str) = key.split('-').next() {
            if let Ok(stock_id) = stock_id_str.parse::<u64>() {
                let history = price_history.entry(stock_id).or_default();
                for candle in candles {
                    history.push((candle.timestamp as i64, candle.close));
                }
            }
        }
    }
    for prices in price_history.values_mut() {
        prices.sort_by_key(|k| k.0);
    }

    let mut mid_prices = HashMap::new();
    let mut spreads = HashMap::new();
    for stock in market_state.stocks.get_all_stocks() {
        if let Some(mid) = market_state.get_mid_price(stock.id) {
            mid_prices.insert(stock.id.to_string(), mid as f64 / 100.0);
        }
        if let Some(spread) = market_state.get_spread(stock.id) {
            spreads.insert(stock.id.to_string(), spread as f64 / 100.0);
        }
    }

    let initial_payload = json!({
        "type": "snapshot",
        "market_state": {
            "order_books": market_state.order_books,
            "stocks": market_state.stocks,
            "last_traded_price": market_state.last_traded_price,
            "cumulative_volume": market_state.cumulative_volume,
            "mid_prices": mid_prices,
            "spreads": spreads,
        },
        "candle_data": candle_history,
        "price_history": price_history,
    });
    if sender
        .send(Message::Text(initial_payload.to_string()))
        .await
        .is_err()
    {
        return; // Client disconnected
    }

    let mut rx = state.tx.subscribe();

    loop {
        tokio::select! {
            // Forward broadcast messages to the client
            Ok(msg) = rx.recv() => {
                if sender.send(Message::Text(msg)).await.is_err() {
                    break; // Client disconnected
                }
            }
            // Handle messages from the client
            Some(Ok(msg)) = receiver.next() => {
                if let Message::Text(text) = msg {
                    if let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) {
                        if client_msg.r#type == "get_history" {
                            if let Some(stock_id) = client_msg.stock_id {
                                let candle_history: HashMap<String, Vec<Candle>> = state.candle_handle
                                    .iter()
                                    .filter(|entry| entry.key().0 == stock_id)
                                    .map(|entry| {
                                        let (id, timeframe) = entry.key();
                                        (format!("{}-{}", id, timeframe), entry.value().clone().into_iter().collect())
                                    })
                                    .collect();

                                let mut price_history: HashMap<u64, Vec<(i64, f64)>> = HashMap::new();
                                for candles in candle_history.values() {
                                    let history = price_history.entry(stock_id).or_default();
                                    for candle in candles {
                                        history.push((candle.timestamp as i64, candle.close));
                                    }
                                }
                                for prices in price_history.values_mut() {
                                    prices.sort_by_key(|k| k.0);
                                }

                                let history_payload = json!({
                                    "type": "history_snapshot",
                                    "stock_id": stock_id,
                                    "candle_data": candle_history,
                                    "price_history": price_history,
                                });

                                if sender.send(Message::Text(history_payload.to_string())).await.is_err() {
                                    break; // Client disconnected
                                }
                            }
                        }
                    }
                } else if matches!(msg, Message::Close(_)) {
                    break;
                }
            }
        }
    }
}
