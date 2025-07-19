// src/agents/panic_agent.rs
//! Panic Agent - Creates feedback loops by forced selling during market stress
//! Simulates margin calls, redemption pressure, and momentum selling

use super::agent_trait::Agent;
use crate::simulation::orchestra::{MarketState, ShadowBookHandle};
use crate::types::order::{Order, OrderRequest, Side, Trade};
use crossbeam_channel::{Receiver, Sender};
use rand::Rng;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;

const PANIC_THRESHOLD_DROP: f64 = 0.05; // 5% price drop triggers panic
const PANIC_AMPLIFICATION: f64 = 2.0; // Panic selling is 2x the original drop
const PANIC_VOLUME_MIN: u64 = 10_000;
const PANIC_VOLUME_MAX: u64 = 50_000;

#[derive(Clone)]
pub struct PanicAgent {
    id: usize,
    monitored_stocks: Vec<u64>, // Stocks this agent monitors for panic

    // Communication channels
    order_channel: Sender<OrderRequest>,
    ack_channel: Arc<Mutex<Receiver<Order>>>,
    port_channel: Arc<Mutex<Receiver<Trade>>>,
    view_handle: ShadowBookHandle,

    // State
    inventory: Arc<RwLock<HashMap<u64, i64>>>,
    cash: Arc<RwLock<f64>>,
    baseline_prices: Arc<RwLock<HashMap<u64, f64>>>, // Track initial prices
    panic_mode: Arc<RwLock<HashMap<u64, bool>>>,     // Which stocks are in panic
}

impl PanicAgent {
    pub fn new(
        id: usize,
        monitored_stocks: Vec<u64>,
        order_channel: Sender<OrderRequest>,
        ack_channel: Receiver<Order>,
        port_channel: Receiver<Trade>,
        view_handle: ShadowBookHandle,
    ) -> Self {
        let mut inventory = HashMap::new();
        let mut baseline_prices = HashMap::new();
        let mut panic_mode = HashMap::new();

        // Initialize tracking for monitored stocks
        for &stock_id in &monitored_stocks {
            inventory.insert(stock_id, 0i64);
            baseline_prices.insert(stock_id, 0.0); // Will be set on first run
            panic_mode.insert(stock_id, false);
        }

        println!(
            "[Panic Agent {}] Monitoring {} stocks for feedback loops",
            id,
            monitored_stocks.len()
        );

        Self {
            id,
            monitored_stocks,
            order_channel,
            ack_channel: Arc::new(Mutex::new(ack_channel)),
            port_channel: Arc::new(Mutex::new(port_channel)),
            view_handle,
            inventory: Arc::new(RwLock::new(inventory)),
            cash: Arc::new(RwLock::new(1_000_000_000.0)), // $1B cash
            baseline_prices: Arc::new(RwLock::new(baseline_prices)),
            panic_mode: Arc::new(RwLock::new(panic_mode)),
        }
    }

    fn update_baseline_prices(&self, view: &MarketState) {
        let mut baselines = self.baseline_prices.write().unwrap();

        for &stock_id in &self.monitored_stocks {
            let current_baseline = baselines.get(&stock_id).copied().unwrap_or(0.0);

            if current_baseline == 0.0 {
                // First time - set baseline
                let price = view
                    .last_traded_price
                    .get(&stock_id)
                    .copied()
                    .or_else(|| {
                        view.stocks
                            .get_stock_by_id(stock_id)
                            .map(|s| s.initial_price)
                    })
                    .unwrap_or(0.0);
                baselines.insert(stock_id, price);
                println!(
                    "[Panic Agent {}] Set baseline for stock {}: ${:.2}",
                    self.id, stock_id, price
                );
            }
        }
    }

    fn check_for_panic_triggers(&self, view: &MarketState) -> Vec<(u64, f64)> {
        let mut panic_triggers = Vec::new();
        let baselines = self.baseline_prices.read().unwrap();

        for &stock_id in &self.monitored_stocks {
            let baseline = baselines.get(&stock_id).copied().unwrap_or(0.0);
            if baseline == 0.0 {
                continue;
            }

            let current_price = view
                .last_traded_price
                .get(&stock_id)
                .copied()
                .or_else(|| {
                    view.stocks
                        .get_stock_by_id(stock_id)
                        .map(|s| s.initial_price)
                })
                .unwrap_or(0.0);

            let price_change = (current_price - baseline) / baseline;

            // Check for significant drops
            if price_change <= -PANIC_THRESHOLD_DROP {
                panic_triggers.push((stock_id, price_change));
                println!(
                    "[Panic Agent {}] PANIC TRIGGER: Stock {} dropped {:.1}% from baseline",
                    self.id,
                    stock_id,
                    price_change * 100.0
                );
            }
        }

        panic_triggers
    }

    fn execute_panic_selling(&self, stock_id: u64, price_drop: f64) {
        // Amplified selling based on price drop magnitude
        let panic_intensity = (price_drop.abs() / PANIC_THRESHOLD_DROP).min(5.0);
        let base_volume = rand::thread_rng().gen_range(PANIC_VOLUME_MIN..=PANIC_VOLUME_MAX);
        let panic_volume = (base_volume as f64 * panic_intensity * PANIC_AMPLIFICATION) as u64;

        // Execute panic selling
        self.order_channel
            .send(OrderRequest::MarketOrder {
                order_id: 0,
                agent_id: self.id,
                stock_id,
                side: Side::Sell,
                volume: panic_volume,
            })
            .ok();

        // Set panic mode
        self.panic_mode.write().unwrap().insert(stock_id, true);

        println!(
            "[Panic Agent {}] 🚨 PANIC SELLING: {} shares of stock {} ({}% drop triggers {}x amplification)",
            self.id,
            panic_volume,
            stock_id,
            price_drop.abs() * 100.0,
            panic_intensity
        );
    }

    fn check_panic_recovery(&self, view: &MarketState) {
        let mut panic_mode = self.panic_mode.write().unwrap();
        let baselines = self.baseline_prices.read().unwrap();

        for &stock_id in &self.monitored_stocks {
            if !panic_mode.get(&stock_id).copied().unwrap_or(false) {
                continue;
            }

            let baseline = baselines.get(&stock_id).copied().unwrap_or(0.0);
            let current_price = view
                .last_traded_price
                .get(&stock_id)
                .copied()
                .or_else(|| {
                    view.stocks
                        .get_stock_by_id(stock_id)
                        .map(|s| s.initial_price)
                })
                .unwrap_or(0.0);

            let price_change = (current_price - baseline) / baseline;

            // Recovery if price is back within 2% of baseline
            if price_change > -0.02 {
                panic_mode.insert(stock_id, false);
                println!(
                    "[Panic Agent {}] 📈 PANIC RECOVERY: Stock {} back to normal",
                    self.id, stock_id
                );
            }
        }
    }
}

impl Agent for PanicAgent {
    fn run(&mut self) {
        // Start portfolio updater thread
        let port_rx_handle = self.port_channel.clone();
        let inventory_handle = self.inventory.clone();
        let cash_handle = self.cash.clone();
        let agent_id = self.id;

        thread::spawn(move || {
            let rx = port_rx_handle.lock().unwrap();
            while let Ok(trade) = rx.recv() {
                if trade.taker_agent_id == agent_id || trade.maker_agent_id == agent_id {
                    let mut inventory_lock = inventory_handle.write().unwrap();
                    let mut cash_lock = cash_handle.write().unwrap();

                    let vol_delta = if trade.taker_agent_id == agent_id {
                        if trade.taker_side == Side::Buy {
                            trade.volume as i64
                        } else {
                            -(trade.volume as i64)
                        }
                    } else {
                        if trade.taker_side == Side::Sell {
                            trade.volume as i64
                        } else {
                            -(trade.volume as i64)
                        }
                    };

                    *inventory_lock.entry(trade.stock_id).or_insert(0) += vol_delta;
                    *cash_lock -= vol_delta as f64 * (trade.price as f64 / 100.0);
                }
            }
        });

        // Start ACK listener thread
        let ack_rx_handle = self.ack_channel.clone();
        thread::spawn(move || {
            let rx = ack_rx_handle.lock().unwrap();
            while let Ok(_order) = rx.recv() {
                // Panic agents don't track orders
            }
        });

        // Main panic monitoring loop
        loop {
            self.decide_actions();
            thread::sleep(std::time::Duration::from_millis(50)); // Check every 50ms
        }
    }

    fn decide_actions(&mut self) {
        let view = self.view_handle.read().unwrap();

        // Update baseline prices if needed
        self.update_baseline_prices(&view);

        // Check for panic triggers
        let panic_triggers = self.check_for_panic_triggers(&view);

        // Release view lock before trading
        drop(view);

        // Execute panic selling for triggered stocks
        for (stock_id, price_drop) in panic_triggers {
            self.execute_panic_selling(stock_id, price_drop);
        }

        // Re-acquire view for panic recovery check
        let view = self.view_handle.read().unwrap();
        self.check_panic_recovery(&view);
    }

    // Required trait methods (simplified)
    fn buy_stock(&mut self, stock_id: u64, volume: u64) {
        self.order_channel
            .send(OrderRequest::MarketOrder {
                order_id: 0,
                agent_id: self.id,
                stock_id,
                side: Side::Buy,
                volume,
            })
            .ok();
    }

    fn sell_stock(&mut self, stock_id: u64, volume: u64) {
        self.order_channel
            .send(OrderRequest::MarketOrder {
                order_id: 0,
                agent_id: self.id,
                stock_id,
                side: Side::Sell,
                volume,
            })
            .ok();
    }

    fn margin_call(&mut self) {}
    fn acknowledge_order(&mut self) {}
    fn update_portfolio(&mut self) {}
    fn get_pending_orders(&self) -> Vec<Order> {
        Vec::new()
    }
    fn cancel_open_order(&mut self, _id: u64) {}

    fn get_id(&self) -> usize {
        self.id
    }

    fn get_inventory(&self) -> i64 {
        self.inventory.read().unwrap().values().sum()
    }

    fn clone_agent(&self) -> Box<dyn Agent> {
        Box::new(self.clone())
    }

    fn evaluate_port(&mut self, view: &MarketState) -> f64 {
        let inventory_lock = self.inventory.read().unwrap();
        let portfolio_value = inventory_lock.iter().fold(0.0, |acc, (stock_id, &vol)| {
            let price = view
                .last_traded_price
                .get(stock_id)
                .copied()
                .or_else(|| {
                    view.stocks
                        .get_stock_by_id(*stock_id)
                        .map(|s| s.initial_price)
                })
                .unwrap_or(0.0);
            acc + (vol as f64 * price)
        });

        let cash = *self.cash.read().unwrap();
        portfolio_value + cash
    }
}
