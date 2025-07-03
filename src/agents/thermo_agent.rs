// src/agents/thermo_agent.rs

use crate::agents::agent_trait::{Agent, MarketView};
use crate::events::MarketEvent;
use crate::simulation::orchestra::ShadowBookHandle;
use crate::types::order::{Order, OrderRequest, Side, Trade};
use crossbeam_channel::{Receiver, Sender};
use rand::Rng;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

const MOMENTUM_WINDOW: usize = 20; // Number of recent trades to consider for momentum

#[derive(Clone)]
pub struct ThermoAgent {
    id: usize,
    order_channel: Sender<OrderRequest>,
    event_receiver: Receiver<MarketEvent>,

    // Agent's internal state
    temperature: f64,          // Tendency to act (0.0 to 1.0)
    chemical_potential: f64,   // Buy/Sell bias (-1.0 to 1.0)
    specific_heat: f64,        // Resistance to sentiment changes
    last_price: HashMap<u64, f64>,
    price_history: HashMap<u64, VecDeque<f64>>,

    // Legacy channels, kept for compatibility with Orchestra structure
    _ack_channel: Arc<Mutex<Receiver<Order>>>,
    _port_channel: Arc<Mutex<Receiver<Trade>>>,
    _view_handle: ShadowBookHandle,
}

impl ThermoAgent {
    pub fn new(
        id: usize,
        order_channel: Sender<OrderRequest>,
        ack_channel: Receiver<Order>,
        port_channel: Receiver<Trade>,
        event_receiver: Receiver<MarketEvent>,
        view_handle: ShadowBookHandle,
        specific_heat: f64,
    ) -> Self {
        Self {
            id,
            order_channel,
            event_receiver,
            temperature: 0.0, // Start cold
            chemical_potential: 0.0, // Start neutral
            specific_heat: specific_heat.max(0.1), // Avoid division by zero
            last_price: HashMap::new(),
            price_history: HashMap::new(),
            _ack_channel: Arc::new(Mutex::new(ack_channel)),
            _port_channel: Arc::new(Mutex::new(port_channel)),
            _view_handle: view_handle,
        }
    }

    // --- Event Handlers ---

    fn handle_sentiment_update(&mut self, _stock_id: u64, score: f64) {
        // Inject heat: sentiment score raises temperature.
        // High specific heat means less change in temperature.
        self.temperature += score / self.specific_heat;
        // Clamp temperature to a valid probability range [0, 1]
        self.temperature = self.temperature.max(0.0).min(1.0);
    }

    fn handle_trade(&mut self, trade: &Trade) {
        // Update price history for momentum calculation
        let history = self.price_history.entry(trade.stock_id).or_insert_with(|| VecDeque::with_capacity(MOMENTUM_WINDOW));
        if history.len() == MOMENTUM_WINDOW {
            history.pop_front();
        }
        let current_price = trade.price as f64 / 100.0;
        history.push_back(current_price);
        self.last_price.insert(trade.stock_id, current_price);

        // Update chemical potential based on momentum
        if history.len() < MOMENTUM_WINDOW / 2 { return; } // Not enough data

        let avg_price: f64 = history.iter().sum::<f64>() / history.len() as f64;
        let momentum = (current_price - avg_price) / avg_price;

        // Update chemical potential, pushing it towards -1 or 1 based on momentum
        // The factor (e.g., 5.0) controls how sensitive the agent is to momentum.
        self.chemical_potential += momentum * 5.0;
        self.chemical_potential = self.chemical_potential.max(-1.0).min(1.0);
    }

    fn handle_heartbeat(&mut self) {
        // On each heartbeat, there's a chance to act, determined by temperature
        let mut rng = rand::thread_rng();
        if rng.gen_bool(self.temperature) {
            // The agent decides to act. Now, what to do?
            // We use the chemical potential to decide between buying and selling.
            let buy_prob = 0.5 * (1.0 + self.chemical_potential);

            let side = if rng.gen_bool(buy_prob) {
                Side::Buy
            } else {
                Side::Sell
            };

            // For now, act on a random stock the agent knows about.
            let known_stocks: Vec<u64> = self.last_price.keys().cloned().collect();
            if !known_stocks.is_empty() {
                if let Some(&stock_id) = known_stocks.get(rng.gen_range(0..known_stocks.len())) {
                    let volume = rng.gen_range(100..=500);
                    let order = OrderRequest::MarketOrder {
                        agent_id: self.id,
                        stock_id,
                        side,
                        volume,
                    };

                    if self.order_channel.send(order).is_err() {
                        eprintln!("[ThermoAgent {}] Failed to send order, market channel closed.", self.id);
                    }
                }
            }
        }

        // Apply thermodynamic cooling
        self.temperature *= 0.995; // Slowly decay temperature over time
        self.chemical_potential *= 0.95; // Slowly decay bias towards neutral
    }
}

impl Agent for ThermoAgent {
    fn run(&mut self) {
        // The new event-driven core loop.
        // Use recv() to block until an event is received, allowing mutable access to self.
        while let Ok(event) = self.event_receiver.recv() {
            match event {
                MarketEvent::SentimentUpdate { stock_id, score } => {
                    self.handle_sentiment_update(stock_id, score);
                }
                MarketEvent::TradeOccurred(trade) => {
                    self.handle_trade(&trade);
                }
                MarketEvent::Heartbeat => {
                    self.handle_heartbeat();
                }
            }
        }
    }

    // --- Legacy trait methods (mostly no-ops) ---
    fn decide_actions(&mut self) {}
    fn buy_stock(&mut self, _stock_id: u64, _volume: u64) {}
    fn sell_stock(&mut self, _stock_id: u64, _volume: u64) {}
    fn acknowledge_order(&mut self) {}
    fn margin_call(&mut self) {}
    fn update_portfolio(&mut self) {}
    fn evaluate_port(&mut self, _market_view: &MarketView) -> f64 { 0.0 }
    fn get_pending_orders(&self) -> Vec<Order> { vec![] }
    fn cancel_open_order(&mut self, _order_id: u64) {}
    fn get_id(&self) -> usize { self.id }
    fn get_inventory(&self) -> i64 { 0 }
    fn clone_agent(&self) -> Box<dyn Agent> {
        Box::new(self.clone())
    }
}