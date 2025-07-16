// src/agents/thermo_agent.rs

use crate::agents::agent_trait::Agent;
use crate::agents::config::{
    THERMO_AGENT_BASE_VOLUME_MAX, THERMO_AGENT_BASE_VOLUME_MIN, THERMO_AGENT_INITIAL_CASH,
    THERMO_AGENT_MIN_TEMP,
};
use crate::events::MarketEvent;
use crate::simulation::orchestra::{MarketState, ShadowBookHandle};
use crate::stocks::StockMarket;
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

    // Agent's internal state (stock-specific)
    temperature: HashMap<u64, f64>, // stock_id -> Tendency to act (0.0 to 1.0)
    chemical_potential: HashMap<u64, f64>, // stock_id -> Buy/Sell bias (-1.0 to 1.0)
    specific_heat: f64,             // Resistance to sentiment changes (global for agent type)

    // Financial state
    cash: f64,
    inventory: HashMap<u64, i64>, // stock_id -> shares owned (positive) or shorted (negative)
    open_orders: HashMap<u64, Order>, // order_id -> Order

    // Price tracking for momentum
    last_price: HashMap<u64, f64>,
    price_history: HashMap<u64, VecDeque<f64>>,

    // Legacy channels, now actively used for financial state updates
    ack_channel: Arc<Mutex<Receiver<Order>>>,
    port_channel: Arc<Mutex<Receiver<Trade>>>,
    _view_handle: ShadowBookHandle,
    stock_market: StockMarket,
}

impl ThermoAgent {
    pub fn new(
        id: usize,
        order_channel: Sender<OrderRequest>,
        ack_channel: Receiver<Order>,
        port_channel: Receiver<Trade>,
        event_receiver: Receiver<MarketEvent>,
        view_handle: ShadowBookHandle,
        stock_market: StockMarket, // Pass StockMarket here
        initial_temperature: f64,
        specific_heat: f64,
        initial_chemical_potential: f64,
    ) -> Self {
        let mut last_price = HashMap::new();
        let mut price_history = HashMap::new();
        let mut temperature_map = HashMap::new();
        let mut chemical_potential_map = HashMap::new();

        // Initialize with initial prices from StockMarket
        for stock in stock_market.get_all_stocks() {
            let initial_px = stock.initial_price;
            last_price.insert(stock.id, initial_px);
            let mut history_deque = VecDeque::with_capacity(MOMENTUM_WINDOW);
            history_deque.push_back(initial_px);
            price_history.insert(stock.id, history_deque);
            temperature_map.insert(stock.id, initial_temperature.max(0.0).min(1.0));
            chemical_potential_map.insert(stock.id, initial_chemical_potential.max(-1.0).min(1.0));
        }

        Self {
            id,
            order_channel,
            event_receiver,
            temperature: temperature_map, // Will be populated per stock
            chemical_potential: chemical_potential_map, // Will be populated per stock
            specific_heat: specific_heat.max(0.1),
            cash: THERMO_AGENT_INITIAL_CASH, // Starting cash
            inventory: HashMap::new(),
            open_orders: HashMap::new(),
            last_price,
            price_history,
            ack_channel: Arc::new(Mutex::new(ack_channel)),
            port_channel: Arc::new(Mutex::new(port_channel)),
            _view_handle: view_handle,
            stock_market,
        }
    }

    // --- Event Handlers ---

    fn handle_sentiment_update(&mut self, stock_id: u64, score: f64) {
        // Inject heat: sentiment score raises temperature for the specific stock.
        // High specific heat means less change in temperature.
        let current_temp = *self.temperature.entry(stock_id).or_insert(0.0);
        let new_temp = current_temp + (score.abs() / self.specific_heat);
        self.temperature
            .insert(stock_id, new_temp.max(0.0).min(1.0));

        // Update chemical potential based on the sentiment score (sign and magnitude).
        // This creates the buy/sell pressure.
        let current_chem_pot = *self.chemical_potential.entry(stock_id).or_insert(0.0);
        let new_chem_pot = current_chem_pot + score; // Add the score as an impulse
        self.chemical_potential
            .insert(stock_id, new_chem_pot.max(-1.0).min(1.0));
    }

    fn handle_trade(&mut self, trade: &Trade) {
        // Update price history for momentum calculation
        let history = self
            .price_history
            .entry(trade.stock_id)
            .or_insert_with(|| VecDeque::with_capacity(MOMENTUM_WINDOW));
        if history.len() == MOMENTUM_WINDOW {
            history.pop_front();
        }
        let current_price = trade.price as f64 / 100.0;
        history.push_back(current_price);
        self.last_price.insert(trade.stock_id, current_price);

        // NOTE: Chemical potential is now driven by sentiment, not trade-based momentum.
    }

    fn handle_heartbeat(&mut self) {
        let mut rng = rand::thread_rng();

        // Iterate over all stocks the agent has some temperature for
        let stocks_to_consider: Vec<u64> = self.stock_market.get_all_ids(); // Use stock_market to get all stock IDs

        for &stock_id in &stocks_to_consider {
            let current_temp = *self.temperature.get(&stock_id).unwrap_or(&0.0);
            let current_chem_pot = *self.chemical_potential.get(&stock_id).unwrap_or(&0.0);

            // Decide IF to act for this stock based on its temperature
            if rng.gen_bool(current_temp) {
                // The agent decides to act. Now, what to do for this stock?
                // Use the chemical potential to decide between buying and selling.
                let buy_prob = 0.5 * (1.0 + current_chem_pot);

                let side = if rng.gen_bool(buy_prob) {
                    Side::Buy
                } else {
                    Side::Sell
                };

                // Determine volume based on cash/inventory and price
                if let Some(&current_price) = self.last_price.get(&stock_id) {
                    let mut volume =
                        rng.gen_range(THERMO_AGENT_BASE_VOLUME_MIN..=THERMO_AGENT_BASE_VOLUME_MAX); // Base volume
                    let cost = volume as f64 * current_price;

                    match side {
                        Side::Buy => {
                            // Ensure enough cash to buy
                            if self.cash < cost {
                                volume = (self.cash / current_price).floor() as u64;
                                if volume == 0 {
                                    println!(
                                        "Chapter 11 Bankruptcy! Not enough cash to buy stock {}",
                                        stock_id
                                    );
                                    continue;
                                } // Cannot afford
                            }
                        }
                        Side::Sell => {
                            // Ensure enough inventory to sell
                            let current_stock_inv = *self.inventory.get(&stock_id).unwrap_or(&0);
                            if current_stock_inv < volume as i64 {
                                volume = current_stock_inv.max(0) as u64;
                                if volume == 0 {
                                    continue;
                                } // No stock to sell
                            }
                        }
                    }

                    let order = OrderRequest::MarketOrder {
                        order_id: 0,
                        agent_id: self.id,
                        stock_id,
                        side,
                        volume,
                    };

                    if self.order_channel.send(order).is_err() {
                        eprintln!(
                            "[ThermoAgent {}] Failed to send order for stock {}: market channel closed.",
                            self.id, stock_id
                        );
                    }
                }
            }

            // Apply thermodynamic cooling for this stock
            let new_temp = current_temp * 0.995;
            self.temperature
                .insert(stock_id, new_temp.max(THERMO_AGENT_MIN_TEMP));
            let new_chem_pot = current_chem_pot * 0.95;
            self.chemical_potential.insert(stock_id, new_chem_pot);
        }
    }

    // --- Financial State Management ---

    fn process_acknowledgements(&mut self) {
        let rx = self.ack_channel.lock().unwrap();
        while let Ok(order) = rx.try_recv() {
            self.open_orders.insert(order.id, order);
        }
    }

    fn process_portfolio_updates(&mut self) {
        let rx = self.port_channel.lock().unwrap();
        while let Ok(tr) = rx.try_recv() {
            if tr.taker_agent_id == self.id || tr.maker_agent_id == self.id {
                let trade_value = (tr.volume as f64 * tr.price as f64) / 100.0;
                let vol_delta = if tr.taker_agent_id == self.id {
                    if tr.taker_side == Side::Buy {
                        self.cash -= trade_value;
                        tr.volume as i64
                    } else {
                        self.cash += trade_value;
                        -(tr.volume as i64)
                    }
                } else {
                    // This agent was the maker
                    if tr.taker_side == Side::Sell {
                        self.cash -= trade_value;
                        tr.volume as i64
                    } else {
                        self.cash += trade_value;
                        -(tr.volume as i64)
                    }
                };

                *self.inventory.entry(tr.stock_id).or_insert(0) += vol_delta;

                if tr.maker_agent_id == self.id {
                    if let Some(order) = self.open_orders.get_mut(&tr.maker_order_id) {
                        order.filled += tr.volume;
                        if order.filled >= order.volume {
                            self.open_orders.remove(&tr.maker_order_id);
                        }
                    }
                }
            }
        }
    }
}

impl Agent for ThermoAgent {
    fn run(&mut self) {
        // The new event-driven core loop.
        //sleep for 10 seconds before joining
        std::thread::sleep(std::time::Duration::from_secs(10));
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
            // Always process acknowledgements and portfolio updates after any event
            self.process_acknowledgements();
            self.process_portfolio_updates();
        }
    }

    // --- Legacy trait methods (mostly no-ops, but now some are meaningful) ---
    fn decide_actions(&mut self) {}
    fn buy_stock(&mut self, _stock_id: u64, _volume: u64) {
        // This agent uses its own internal logic to decide when to buy/sell
    }
    fn sell_stock(&mut self, _stock_id: u64, _volume: u64) {
        // This agent uses its own internal logic to decide when to buy/sell
    }
    fn acknowledge_order(&mut self) {
        // Handled internally by process_acknowledgements
    }
    fn margin_call(&mut self) {
        // Basic margin call logic: if cash is very low, try to sell some stock
        if self.cash < crate::agents::config::THERMO_AGENT_MARGIN_CALL_THRESHOLD {
            // Example threshold
            for (&stock_id, &volume) in self.inventory.iter() {
                if volume > 0 {
                    // If we own stock
                    let order = OrderRequest::MarketOrder {
                        order_id: 0,
                        agent_id: self.id,
                        stock_id,
                        side: Side::Sell,
                        volume: volume as u64,
                    };
                    if self.order_channel.send(order).is_err() {
                        eprintln!(
                            "[ThermoAgent {}] Failed to send margin call sell order.",
                            self.id
                        );
                    }
                }
            }
        }
    }
    fn update_portfolio(&mut self) {
        // Handled internally by process_portfolio_updates
    }
    fn evaluate_port(&mut self, market_view: &MarketState) -> f64 {
        let mut current_value = self.cash;
        for (&stock_id, &volume) in self.inventory.iter() {
            if let Some(px) = market_view.get_mid_price(stock_id) {
                current_value += volume as f64 * (px as f64 / 100.0);
            }
        }
        current_value
    }
    fn get_pending_orders(&self) -> Vec<Order> {
        self.open_orders.values().cloned().collect()
    }
    fn cancel_open_order(&mut self, order_id: u64) {
        // Send a cancel request to the market
        let order_request = OrderRequest::CancelOrder {
            agent_id: self.id,
            order_id,
        };
        if self.order_channel.send(order_request).is_err() {
            eprintln!(
                "[ThermoAgent {}] Failed to send cancel order request.",
                self.id
            );
        }
        // Optimistically remove from our local tracking
        self.open_orders.remove(&order_id);
    }
    fn get_id(&self) -> usize {
        self.id
    }
    fn get_inventory(&self) -> i64 {
        self.inventory.values().sum()
    }
    fn clone_agent(&self) -> Box<dyn Agent> {
        Box::new(self.clone())
    }
}
