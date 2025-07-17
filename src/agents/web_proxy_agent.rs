// src/agents/web_proxy_agent.rs

use crate::agents::agent_trait::Agent;
use crate::simulation::orchestra::MarketState;
use crate::types::order::{Order, OrderRequest, Side, Trade};
use crossbeam_channel::{Receiver, Sender};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

// --- Message types for communication between WebServer and WebProxyAgent ---

/// Messages sent from the WebServer to the WebProxyAgent
#[derive(Debug)]
pub enum ProxyRequest {
    Register {
        client_uuid: String,
        response_tx: Sender<ClientResponse>,
    },
    SubmitOrder {
        client_uuid: String,
        stock_id: u64,
        side: Side,
        order_type: String, // "Market" or "Limit"
        volume: u64,
        price: Option<f64>,
    },
}

/// Messages sent from the WebProxyAgent back to the WebServer task
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum ClientResponse {
    OrderAck(Order),
    PortfolioUpdate(SerializablePortfolio),
    TradeUpdate(Trade),
}

// --- Portfolio Management ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Holding {
    pub stock_id: u64,
    pub quantity: u64,
    pub cost_basis: f64, // Average price per share
}

#[derive(Debug, Clone, Default)]
pub struct Portfolio {
    pub cash: f64,
    pub holdings: HashMap<u64, Holding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializablePortfolio {
    pub cash: f64,
    pub holdings: HashMap<u64, Holding>,
}

impl From<&Portfolio> for SerializablePortfolio {
    fn from(portfolio: &Portfolio) -> Self {
        SerializablePortfolio {
            cash: portfolio.cash,
            holdings: portfolio.holdings.clone(),
        }
    }
}

impl Portfolio {
    pub fn new(initial_cash: f64) -> Self {
        Self {
            cash: initial_cash,
            holdings: HashMap::new(),
        }
    }

    pub fn update_on_trade(&mut self, trade: &Trade) {
        let trade_value = (trade.price as f64 / 100.0) * trade.volume as f64;
        match trade.taker_side {
            Side::Buy => {
                self.cash -= trade_value;
                let holding = self.holdings.entry(trade.stock_id).or_insert(Holding {
                    stock_id: trade.stock_id,
                    quantity: 0,
                    cost_basis: 0.0,
                });
                let old_total_value = holding.cost_basis * holding.quantity as f64;
                holding.quantity += trade.volume;
                holding.cost_basis = (old_total_value + trade_value) / holding.quantity as f64;
            }
            Side::Sell => {
                self.cash += trade_value;
                if let Some(holding) = self.holdings.get_mut(&trade.stock_id) {
                    holding.quantity -= trade.volume;
                    if holding.quantity == 0 {
                        self.holdings.remove(&trade.stock_id);
                    }
                }
            }
        }
    }
}

// --- The WebProxyAgent Actor ---

pub struct WebProxyAgent {
    id: usize,
    order_tx: Sender<OrderRequest>,
    ack_rx: Receiver<Order>,
    trade_rx: Receiver<Trade>,
    pub proxy_request_rx: Receiver<ProxyRequest>,
    clients: HashMap<String, Sender<ClientResponse>>,
    portfolios: HashMap<String, Portfolio>,
    order_to_client_map: DashMap<u64, String>,
    client_id_queues: DashMap<u64, VecDeque<String>>,
}

impl WebProxyAgent {
    pub fn new(
        id: usize,
        order_tx: Sender<OrderRequest>,
        ack_rx: Receiver<Order>,
        trade_rx: Receiver<Trade>,
        proxy_request_rx: Receiver<ProxyRequest>,
    ) -> Self {
        Self {
            id,
            order_tx,
            ack_rx,
            trade_rx,
            proxy_request_rx,
            clients: HashMap::new(),
            portfolios: HashMap::new(),
            order_to_client_map: DashMap::new(),
            client_id_queues: DashMap::new(),
        }
    }

    fn handle_proxy_request(&mut self, request: ProxyRequest) {
        match request {
            ProxyRequest::Register {
                client_uuid,
                response_tx,
            } => {
                println!("[WebProxyAgent] Registering client: {}", client_uuid);
                self.clients
                    .insert(client_uuid.clone(), response_tx.clone());
                let portfolio = self
                    .portfolios
                    .entry(client_uuid)
                    .or_insert_with(|| Portfolio::new(10_000.0));

                // Send initial portfolio state
                let _ = response_tx.send(ClientResponse::PortfolioUpdate(
                    SerializablePortfolio::from(&*portfolio),
                ));
            }
            ProxyRequest::SubmitOrder {
                client_uuid,
                stock_id,
                side,
                order_type,
                volume,
                price,
            } => {
                let price_in_cents = price.map_or(0, |p| (p * 100.0).round() as u64);

                let order_request = if order_type.to_lowercase() == "limit" {
                    OrderRequest::LimitOrder {
                        order_id: 0, // Market will assign this
                        agent_id: self.id,
                        stock_id,
                        side,
                        price: price_in_cents,
                        volume,
                    }
                } else {
                    OrderRequest::MarketOrder {
                        order_id: 0, // Market will assign this
                        agent_id: self.id,
                        stock_id,
                        side,
                        volume,
                    }
                };

                if self.order_tx.send(order_request).is_err() {
                    eprintln!(
                        "[WebProxyAgent] Failed to send order to market for client {}",
                        client_uuid
                    );
                } else {
                    self.client_id_queues
                        .entry(stock_id)
                        .or_default()
                        .push_back(client_uuid.clone());
                }
            }
        }
    }

    fn handle_market_ack(&mut self, ack: Order) {
        if let Some(mut queue_lock) = self.client_id_queues.get_mut(&ack.stock_id) {
            if let Some(client_id) = queue_lock.pop_front() {
                self.order_to_client_map.insert(ack.id, client_id.clone());
                if let Some(client_tx) = self.clients.get(&client_id) {
                    if client_tx.send(ClientResponse::OrderAck(ack)).is_err() {
                        eprintln!(
                            "[WebProxyAgent] Failed to send OrderAck to client {}. Client disconnected?",
                            client_id
                        );
                    }
                }
            } else {
                eprintln!(
                    "[WebProxyAgent] Queue for stock_id {} was empty, but received ACK. This should not happen.",
                    ack.stock_id
                );
            }
        } else {
            eprintln!(
                "[WebProxyAgent] No queue found for stock_id {}. This should not happen.",
                ack.stock_id
            );
        }
    }

    fn handle_market_trade(&mut self, trade: Trade) {
        let mut client_ids_to_update: Vec<String> = Vec::new();

        // Check taker side
        if let Some(client_id) = self.order_to_client_map.get(&trade.taker_order_id) {
            client_ids_to_update.push(client_id.clone());
        }

        // Check maker side
        if let Some(client_id) = self.order_to_client_map.get(&trade.maker_order_id) {
            // Only add if not already present (e.g., if taker and maker are the same client, though unlikely in this model)
            if !client_ids_to_update.contains(&*client_id) {
                client_ids_to_update.push(client_id.clone());
            }
        }

        for client_id in client_ids_to_update {
            if let Some(portfolio) = self.portfolios.get_mut(&client_id) {
                portfolio.update_on_trade(&trade);
                if let Some(client_tx) = self.clients.get(&client_id) {
                    if client_tx
                        .send(ClientResponse::PortfolioUpdate(
                            SerializablePortfolio::from(&*portfolio),
                        ))
                        .is_err()
                    {
                        eprintln!(
                            "[WebProxyAgent] Failed to send PortfolioUpdate to client {}. Client disconnected?",
                            client_id
                        );
                    }
                    // Also send the trade update
                    if client_tx.send(ClientResponse::TradeUpdate(trade)).is_err() {
                        eprintln!(
                            "[WebProxyAgent] Failed to send TradeUpdate to client {}. Client disconnected?",
                            client_id
                        );
                    }
                }
            }
        }
    }
}

impl Agent for WebProxyAgent {
    fn run(&mut self) {
        println!("[WebProxyAgent {}] running.", self.id);
        loop {
            crossbeam_channel::select! {
                recv(self.proxy_request_rx) -> msg => {
                    if let Ok(req) = msg {
                        self.handle_proxy_request(req);
                    } else {
                        break; // Channel disconnected
                    }
                },
                recv(self.ack_rx) -> msg => {
                    if let Ok(ack) = msg {
                        self.handle_market_ack(ack);
                    }
                },
                recv(self.trade_rx) -> msg => {
                    if let Ok(trade) = msg {
                        self.handle_market_trade(trade);
                    }
                }
            }
        }
    }

    fn decide_actions(&mut self) {}
    fn get_id(&self) -> usize {
        self.id
    }
    fn clone_agent(&self) -> Box<dyn Agent> {
        Box::new(self.clone())
    }
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
    fn get_inventory(&self) -> i64 {
        0
    }
}

impl Clone for WebProxyAgent {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            order_tx: self.order_tx.clone(),
            ack_rx: self.ack_rx.clone(),
            trade_rx: self.trade_rx.clone(),
            proxy_request_rx: self.proxy_request_rx.clone(),
            clients: self.clients.clone(),
            portfolios: self.portfolios.clone(),
            order_to_client_map: self.order_to_client_map.clone(),
            client_id_queues: self.client_id_queues.clone(),
        }
    }
}
