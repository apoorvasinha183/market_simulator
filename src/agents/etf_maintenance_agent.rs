// src/agents/etf_maintenance_agent.rs
//! ETF Maintenance Agent - Specialized arbitrageur for keeping ETF prices in line with NAV
//! These agents act like authorized participants in real ETF markets

use super::agent_trait::Agent;
use crate::simulation::orchestra::{MarketState, ShadowBookHandle};
use crate::stocks::definitions::{ETFInfo, StockMarket};
use crate::types::order::{Order, OrderRequest, Side, Trade};
use crossbeam_channel::{Receiver, Sender};
use rand::Rng;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;

// Tighter thresholds for maintenance agents
const ETF_MAINTENANCE_THRESHOLD_BPS: u64 = 10; // 0.1% threshold (much tighter than regular arbitrage)
const ETF_CREATION_UNIT_SIZE: u64 = 50_000; // Standard creation unit size
const ETF_MAINTENANCE_VOLUME_MIN: u64 = 5_000;
const ETF_MAINTENANCE_VOLUME_MAX: u64 = 25_000;
const ETF_MAX_INVENTORY_RATIO: f64 = 0.1; // Max 10% of total position in any direction

#[derive(Clone)]
pub struct ETFMaintenanceAgent {
    id: usize,
    etf_stock_id: u64,
    #[allow(dead_code)]
    etf_info: ETFInfo,
    constituent_stock_ids: HashMap<u64, f64>, // stock_id -> weight mapping

    // Communication channels
    order_channel: Sender<OrderRequest>,
    ack_channel: Arc<Mutex<Receiver<Order>>>,
    port_channel: Arc<Mutex<Receiver<Trade>>>,
    view_handle: ShadowBookHandle,

    // State - more sophisticated inventory management
    inventory: Arc<RwLock<HashMap<u64, i64>>>, // stock_id -> position (includes ETF + constituents)
    target_inventory: Arc<RwLock<HashMap<u64, i64>>>, // Target neutral positions
    cash: Arc<RwLock<f64>>,
    last_nav: Arc<RwLock<f64>>,
    last_etf_price: Arc<RwLock<f64>>,
    last_arbitrage_time: Arc<RwLock<std::time::Instant>>,
    total_arbitrage_profit: Arc<RwLock<f64>>,
}

impl ETFMaintenanceAgent {
    pub fn new(
        id: usize,
        etf_stock_id: u64,
        order_channel: Sender<OrderRequest>,
        ack_channel: Receiver<Order>,
        port_channel: Receiver<Trade>,
        view_handle: ShadowBookHandle,
        stock_market: &StockMarket,
    ) -> Option<Self> {
        // Get ETF info from stock market
        let etf_stock = stock_market.get_stock_by_id(etf_stock_id)?;
        let etf_info = stock_market.get_etf_info(&etf_stock.ticker)?.clone();

        // Map constituent tickers to stock IDs
        let mut constituent_stock_ids = HashMap::new();
        for (ticker, weight) in &etf_info.parsed_holdings {
            if let Some(stock) = stock_market.get_stock_by_ticker(ticker) {
                constituent_stock_ids.insert(stock.id, *weight);
            }
        }

        // Initialize inventory and target positions
        let mut inventory = HashMap::new();
        let mut target_inventory = HashMap::new();

        inventory.insert(etf_stock_id, 0i64); // ETF itself
        target_inventory.insert(etf_stock_id, 0i64);

        for &stock_id in constituent_stock_ids.keys() {
            inventory.insert(stock_id, 0i64); // Constituents
            target_inventory.insert(stock_id, 0i64);
        }

        println!(
            "[ETF Maintenance Agent {}] Managing ETF {} ({}) with {} constituents",
            id,
            etf_stock.ticker,
            etf_stock_id,
            constituent_stock_ids.len()
        );
        println!(
            "  - Maintenance threshold: {}bps",
            ETF_MAINTENANCE_THRESHOLD_BPS
        );
        println!("  - Creation unit size: {} shares", ETF_CREATION_UNIT_SIZE);

        Some(Self {
            id,
            etf_stock_id,
            etf_info,
            constituent_stock_ids,
            order_channel,
            ack_channel: Arc::new(Mutex::new(ack_channel)),
            port_channel: Arc::new(Mutex::new(port_channel)),
            view_handle,
            inventory: Arc::new(RwLock::new(inventory)),
            target_inventory: Arc::new(RwLock::new(target_inventory)),
            cash: Arc::new(RwLock::new(10_000_000_000.0)), // $10B starting cash (deep pockets)
            last_nav: Arc::new(RwLock::new(0.0)),
            last_etf_price: Arc::new(RwLock::new(0.0)),
            last_arbitrage_time: Arc::new(RwLock::new(std::time::Instant::now())),
            total_arbitrage_profit: Arc::new(RwLock::new(0.0)),
        })
    }

    fn calculate_nav(&self, view: &MarketState) -> Option<f64> {
        let mut nav = 0.0;

        // First, get the ticker of the ETF this agent is managing
        let etf_ticker = match view.stocks.get_ticker_by_id(self.etf_stock_id) {
            Some(ticker) => ticker,
            None => return None, // This agent's ETF doesn't exist
        };

        // Then, use the ticker to get the ETF's definition (holdings)
        let etf_info = match view.stocks.get_etf_info(etf_ticker) {
            Some(info) => info,
            None => return None, // Not a valid ETF
        };

        // Now, calculate NAV based on the parsed holdings
        for (holding_ticker, &weight) in &etf_info.parsed_holdings {
            let holding_id = match view.stocks.get_id_by_ticker(holding_ticker) {
                Some(id) => id,
                None => continue, // Skip if a holding isn't found in the market
            };

            let price = view
                .last_traded_price
                .get(&holding_id)
                .copied()
                .unwrap_or_else(|| {
                    view.stocks
                        .get_stock_by_id(holding_id)
                        .map(|s| s.initial_price)
                        .unwrap_or(0.0)
                });
            nav += price * weight;
        }

        if nav > 0.0 {
            Some(nav)
        } else {
            None // Can't have a NAV of zero or less
        }
    }

    fn get_etf_price(&self, view: &MarketState) -> f64 {
        view.last_traded_price
            .get(&self.etf_stock_id)
            .copied()
            .unwrap_or_else(|| {
                view.stocks
                    .get_stock_by_id(self.etf_stock_id)
                    .map(|s| s.initial_price)
                    .unwrap_or(0.0)
            })
    }

    fn calculate_maintenance_opportunity(
        &self,
        nav: f64,
        etf_price: f64,
    ) -> Option<(Side, f64, String)> {
        if nav <= 0.0 || etf_price <= 0.0 {
            return None;
        }

        let price_diff_bps = (etf_price - nav) / nav * 10000.0;
        let abs_diff_bps = price_diff_bps.abs();

        if abs_diff_bps < ETF_MAINTENANCE_THRESHOLD_BPS as f64 {
            return None; // Within acceptable range
        }

        let action_type = if etf_price > nav {
            // ETF overpriced: sell ETF, buy constituents (redemption-like)
            (Side::Sell, abs_diff_bps, "REDEMPTION".to_string())
        } else {
            // ETF underpriced: buy ETF, sell constituents (creation-like)
            (Side::Buy, abs_diff_bps, "CREATION".to_string())
        };

        Some(action_type)
    }

    fn check_inventory_limits(&self) -> bool {
        let inventory = self.inventory.read().unwrap();
        let target = self.target_inventory.read().unwrap();

        // Check if any position is too far from target
        for (&stock_id, &current_pos) in inventory.iter() {
            let target_pos = target.get(&stock_id).copied().unwrap_or(0);
            let deviation = (current_pos - target_pos).abs() as f64;

            // If deviation is more than 10% of creation unit size, we're getting risky
            if deviation > (ETF_CREATION_UNIT_SIZE as f64 * ETF_MAX_INVENTORY_RATIO) {
                return false;
            }
        }

        true
    }

    fn execute_maintenance_arbitrage(
        &self,
        etf_side: Side,
        magnitude_bps: f64,
        action_type: String,
    ) {
        // Check inventory limits before trading
        if !self.check_inventory_limits() {
            println!(
                "[ETF Maintenance Agent {}] Skipping arbitrage - inventory limits exceeded",
                self.id
            );
            return;
        }

        // Calculate volume based on magnitude of opportunity
        let base_volume =
            rand::thread_rng().gen_range(ETF_MAINTENANCE_VOLUME_MIN..=ETF_MAINTENANCE_VOLUME_MAX);
        let volume_multiplier = (magnitude_bps / 10.0).min(5.0).max(1.0); // Scale with opportunity size
        let volume = (base_volume as f64 * volume_multiplier) as u64;

        // Trade the ETF
        self.order_channel
            .send(OrderRequest::MarketOrder {
                order_id: 0,
                agent_id: self.id,
                stock_id: self.etf_stock_id,
                side: etf_side,
                volume,
            })
            .ok();

        // Trade constituents in opposite direction with proper weighting
        let constituent_side = etf_side.opposite();
        for (&stock_id, &weight) in &self.constituent_stock_ids {
            let constituent_volume = ((volume as f64 * weight) as u64).max(100);

            self.order_channel
                .send(OrderRequest::MarketOrder {
                    order_id: 0,
                    agent_id: self.id,
                    stock_id,
                    side: constituent_side,
                    volume: constituent_volume,
                })
                .ok();
        }

        // Update arbitrage tracking
        *self.last_arbitrage_time.write().unwrap() = std::time::Instant::now();

        println!(
            "[ETF Maintenance Agent {}] {} Arbitrage: {:?} ETF vs {:?} constituents | {}bps opportunity | Volume: {}",
            self.id, action_type, etf_side, constituent_side, magnitude_bps as u64, volume
        );
    }

    fn rebalance_inventory(&self) {
        // Periodically rebalance inventory back to neutral
        let inventory = self.inventory.read().unwrap();
        let target = self.target_inventory.read().unwrap();

        for (&stock_id, &current_pos) in inventory.iter() {
            let target_pos = target.get(&stock_id).copied().unwrap_or(0);
            let imbalance = current_pos - target_pos;

            if imbalance.abs() > 1000 {
                // Only rebalance significant imbalances
                let side = if imbalance > 0 { Side::Sell } else { Side::Buy };
                let volume = (imbalance.abs() as u64).min(5000); // Gradual rebalancing

                self.order_channel
                    .send(OrderRequest::MarketOrder {
                        order_id: 0,
                        agent_id: self.id,
                        stock_id,
                        side,
                        volume,
                    })
                    .ok();

                println!(
                    "[ETF Maintenance Agent {}] Rebalancing: {:?} {} shares of stock {}",
                    self.id, side, volume, stock_id
                );
            }
        }
    }
}

impl Agent for ETFMaintenanceAgent {
    fn run(&mut self) {
        // Start portfolio updater thread
        let port_rx_handle = self.port_channel.clone();
        let inventory_handle = self.inventory.clone();
        let cash_handle = self.cash.clone();
        let profit_handle = self.total_arbitrage_profit.clone();
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
                    let cash_delta = -(vol_delta as f64 * (trade.price as f64 / 100.0));
                    *cash_lock += cash_delta;

                    // Track arbitrage profit (simplified)
                    if cash_delta > 0.0 {
                        *profit_handle.write().unwrap() += cash_delta;
                    }
                }
            }
        });

        // Start ACK listener thread
        let ack_rx_handle = self.ack_channel.clone();
        thread::spawn(move || {
            let rx = ack_rx_handle.lock().unwrap();
            while let Ok(_order) = rx.recv() {
                // ETF maintenance agents don't need to track open orders
            }
        });

        // Main maintenance loop - much faster than regular ETF agents
        let mut rebalance_counter = 0;
        loop {
            self.decide_actions();

            // Rebalance inventory every 100 cycles
            rebalance_counter += 1;
            if rebalance_counter >= 100 {
                self.rebalance_inventory();
                rebalance_counter = 0;
            }

            thread::sleep(std::time::Duration::from_micros(1)); // Check every 10ms (very fast)
        }
    }

    fn decide_actions(&mut self) {
        let view = self.view_handle.read().unwrap();

        // Calculate current NAV and ETF price
        let nav = match self.calculate_nav(&view) {
            Some(n) => n,
            None => return,
        };

        let etf_price = self.get_etf_price(&view);

        // Update stored values
        *self.last_nav.write().unwrap() = nav;
        *self.last_etf_price.write().unwrap() = etf_price;

        // Check for maintenance opportunity
        if let Some((etf_side, magnitude_bps, action_type)) =
            self.calculate_maintenance_opportunity(nav, etf_price)
        {
            drop(view); // Release the lock before executing trades
            self.execute_maintenance_arbitrage(etf_side, magnitude_bps, action_type);
        }
    }

    // Required trait methods (simplified implementations)
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

    fn margin_call(&mut self) {
        // ETF maintenance agents have deep pockets and sophisticated risk management
    }

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
        let total_profit = *self.total_arbitrage_profit.read().unwrap();

        println!(
            "[ETF Maintenance Agent {}] Portfolio: ${:.2}M | Cash: ${:.2}M | Profit: ${:.2}K",
            self.id,
            portfolio_value / 1_000_000.0,
            cash / 1_000_000.0,
            total_profit / 1_000.0
        );

        portfolio_value + cash
    }
}
