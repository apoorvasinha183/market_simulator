// src/agents/etf_agent.rs

use super::agent_trait::Agent;
use crate::simulation::orchestra::{MarketState, ShadowBookHandle};
use crate::stocks::definitions::{Stock, StockMarket};
use crate::types::order::{Order, OrderRequest, Side, Trade};
use crossbeam_channel::{Receiver, Sender};
use rand::Rng;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;

const ETF_ARBITRAGE_THRESHOLD_BPS: u64 = 50; // 0.5% threshold to trigger arbitrage
const ETF_REBALANCE_VOLUME_MIN: u64 = 1_000;
const ETF_REBALANCE_VOLUME_MAX: u64 = 10_000;

#[derive(Clone)]
pub struct ETFAgent {
    id: usize,
    etf_stock_id: u64, // The ETF's stock ID (e.g., BUBBLE = 21)
    #[allow(dead_code)]
    etf_info: Stock, // ETF metadata with holdings
    constituent_stock_ids: HashMap<u64, f64>, // stock_id -> weight mapping

    // Communication channels
    order_channel: Sender<OrderRequest>,
    ack_channel: Arc<Mutex<Receiver<Order>>>,
    port_channel: Arc<Mutex<Receiver<Trade>>>,
    view_handle: ShadowBookHandle,

    // State
    inventory: Arc<RwLock<HashMap<u64, i64>>>, // stock_id -> position (includes ETF itself)
    cash: Arc<RwLock<f64>>,
    last_nav: Arc<RwLock<f64>>,
    last_etf_price: Arc<RwLock<f64>>,
}

impl ETFAgent {
    pub fn new(
        id: usize,
        etf_stock_id: u64,
        order_channel: Sender<OrderRequest>,
        ack_channel: Receiver<Order>,
        port_channel: Receiver<Trade>,
        view_handle: ShadowBookHandle,
        stock_market: &StockMarket,
        initial_cash: f64,
    ) -> Option<Self> {
        let etf_stock = stock_market.get_stock_by_id(etf_stock_id)?.clone();
        if !etf_stock.is_etf() {
            return None;
        }

        // Map constituent tickers to stock IDs
        let mut constituent_stock_ids = HashMap::new();
        for (ticker, weight) in &etf_stock.parsed_holdings {
            if let Some(stock) = stock_market.get_stock_by_ticker(ticker) {
                constituent_stock_ids.insert(stock.id, *weight);
            }
        }

        // Initialize inventory (start with 0 for all stocks)
        let mut inventory = HashMap::new();
        inventory.insert(etf_stock_id, 0i64); // ETF itself
        for &stock_id in constituent_stock_ids.keys() {
            inventory.insert(stock_id, 0i64); // Constituents
        }

        println!(
            "[ETF Agent {}] Managing ETF {} with {} constituents",
            id,
            etf_stock.ticker,
            constituent_stock_ids.len()
        );

        Some(Self {
            id,
            etf_stock_id,
            etf_info: etf_stock,
            constituent_stock_ids,
            order_channel,
            ack_channel: Arc::new(Mutex::new(ack_channel)),
            port_channel: Arc::new(Mutex::new(port_channel)),
            view_handle,
            inventory: Arc::new(RwLock::new(inventory)),
            cash: Arc::new(RwLock::new(initial_cash)),
            last_nav: Arc::new(RwLock::new(0.0)),
            last_etf_price: Arc::new(RwLock::new(0.0)),
        })
    }

    fn calculate_nav(&self, view: &MarketState) -> Option<f64> {
        let mut nav = 0.0;

        // Now, calculate NAV based on the parsed holdings
        for (holding_ticker, &weight) in &self.etf_info.parsed_holdings {
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
                // Fallback to initial price if no trades yet
                view.stocks
                    .get_stock_by_id(self.etf_stock_id)
                    .map(|s| s.initial_price)
                    .unwrap_or(0.0)
            })
    }

    fn calculate_arbitrage_opportunity(&self, nav: f64, etf_price: f64) -> Option<(Side, f64)> {
        if nav <= 0.0 || etf_price <= 0.0 {
            return None;
        }

        let price_diff_bps = ((etf_price - nav) / nav * 10000.0).abs();

        if price_diff_bps < ETF_ARBITRAGE_THRESHOLD_BPS as f64 {
            return None; // No arbitrage opportunity
        }

        if etf_price > nav {
            // ETF is overpriced: sell ETF, buy constituents
            Some((Side::Sell, price_diff_bps))
        } else {
            // ETF is underpriced: buy ETF, sell constituents
            Some((Side::Buy, price_diff_bps))
        }
    }

    fn execute_arbitrage(&self, etf_side: Side, _magnitude_bps: f64) {
        let volume =
            rand::thread_rng().gen_range(ETF_REBALANCE_VOLUME_MIN..=ETF_REBALANCE_VOLUME_MAX);

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

        // Trade constituents in opposite direction
        let constituent_side = etf_side.opposite();
        for (&stock_id, &weight) in &self.constituent_stock_ids {
            let constituent_volume = ((volume as f64 * weight) as u64).max(100); // Minimum 100 shares

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

        println!(
            "[ETF Agent {}] Arbitrage: {:?} ETF, {:?} constituents ({}bps opportunity)",
            self.id, etf_side, constituent_side, _magnitude_bps as u64
        );
    }
}

impl Agent for ETFAgent {
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
                        // This agent was the maker
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
                // ETF agents don't need to track open orders for now
            }
        });

        // Main arbitrage loop
        loop {
            self.decide_actions();
            thread::sleep(std::time::Duration::from_micros(100)); // Check every 100ms
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

        // Check for arbitrage opportunity
        if let Some((etf_side, magnitude_bps)) =
            self.calculate_arbitrage_opportunity(nav, etf_price)
        {
            drop(view); // Release the lock before executing trades
            self.execute_arbitrage(etf_side, magnitude_bps);
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
        // ETF agents have deep pockets, no margin calls for now
    }

    fn acknowledge_order(&mut self) {
        // Handled in separate thread
    }

    fn update_portfolio(&mut self) {
        // Handled in separate thread
    }

    fn get_pending_orders(&self) -> Vec<Order> {
        Vec::new() // Simplified
    }

    fn cancel_open_order(&mut self, _id: u64) {
        // Simplified
    }

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
