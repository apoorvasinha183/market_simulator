// src/agents/astrologer_agent.rs

use crossbeam_channel::{Receiver, Sender};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};
use std::thread;

use crate::agents::agent_trait::Agent;
use crate::simulation::candle_analyzer::CandleDataHandle;
use crate::simulation::orchestra::MarketState;
use crate::types::candle::{Candle, TimeFrame};
use crate::types::order::{Order, OrderRequest, Side, Trade};

const ASTROLOGER_INITIAL_CASH: f64 = 1_000_000.0;
const TRADE_SIZE_PERCENT: f64 = 0.1; // Trade 10% of available cash/inventory

/// An agent that makes trading decisions based on technical analysis of candlestick data.
/// This version is parallelized and manages its own portfolio.
#[derive(Clone)]
pub struct AstrologerAgent {
    id: usize,
    order_channel: Sender<OrderRequest>,
    ack_channel: Arc<Receiver<Order>>,
    port_channel: Arc<Receiver<Trade>>,
    candle_data: CandleDataHandle,

    // Thread-safe state
    cash: Arc<RwLock<f64>>,
    inventory: Arc<RwLock<HashMap<u64, i64>>>,
    open_orders: Arc<RwLock<HashMap<u64, Order>>>,
}

impl AstrologerAgent {
    pub fn new(
        id: usize,
        order_channel: Sender<OrderRequest>,
        ack_channel: Receiver<Order>,
        port_channel: Receiver<Trade>,
        _view_handle: Arc<RwLock<MarketState>>, // Not used by this agent, but part of the signature
        candle_data: CandleDataHandle,
    ) -> Self {
        Self {
            id,
            order_channel,
            ack_channel: Arc::new(ack_channel),
            port_channel: Arc::new(port_channel),
            candle_data,
            cash: Arc::new(RwLock::new(ASTROLOGER_INITIAL_CASH)),
            inventory: Arc::new(RwLock::new(HashMap::new())),
            open_orders: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    // --- Internal Worker Functions ---

    fn run_portfolio_updater_internal(
        port_rx: &Receiver<Trade>,
        inventory: &Arc<RwLock<HashMap<u64, i64>>>,
        cash: &Arc<RwLock<f64>>,
        open_orders: &Arc<RwLock<HashMap<u64, Order>>>,
        agent_id: usize,
    ) {
        while let Ok(tr) = port_rx.recv() {
            if tr.taker_agent_id == agent_id || tr.maker_agent_id == agent_id {
                let mut inventory_lock = inventory.write().unwrap();
                let mut cash_lock = cash.write().unwrap();

                let trade_value = (tr.volume as f64 * tr.price as f64) / 100.0;
                let vol_delta = if tr.taker_agent_id == agent_id {
                    if tr.taker_side == Side::Buy {
                        *cash_lock -= trade_value;
                        tr.volume as i64
                    } else {
                        *cash_lock += trade_value;
                        -(tr.volume as i64)
                    }
                } else {
                    // This agent was the maker (unlikely for market orders, but possible)
                    if tr.taker_side == Side::Sell {
                        *cash_lock -= trade_value;
                        tr.volume as i64
                    } else {
                        *cash_lock += trade_value;
                        -(tr.volume as i64)
                    }
                };

                *inventory_lock.entry(tr.stock_id).or_insert(0) += vol_delta;

                if tr.maker_agent_id == agent_id {
                    let mut open_orders_lock = open_orders.write().unwrap();
                    if let Some(order) = open_orders_lock.get_mut(&tr.maker_order_id) {
                        order.filled += tr.volume;
                        if order.filled >= order.volume {
                            open_orders_lock.remove(&tr.maker_order_id);
                        }
                    }
                }
            }
        }
    }

    fn run_ack_listener_internal(
        ack_rx: &Receiver<Order>,
        open_orders: &Arc<RwLock<HashMap<u64, Order>>>,
    ) {
        while let Ok(order) = ack_rx.recv() {
            open_orders.write().unwrap().insert(order.id, order);
        }
    }

    // --- Technical Analysis Functions ---

    fn calculate_sma(candles: &VecDeque<Candle>, period: usize) -> f64 {
        if candles.len() < period {
            return 0.0;
        }
        let sum: f64 = candles.iter().rev().take(period).map(|c| c.close).sum();
        sum / period as f64
    }

    fn calculate_rsi(candles: &VecDeque<Candle>, period: usize) -> f64 {
        if candles.len() < period + 1 {
            return 50.0; // Neutral RSI
        }

        let mut gains = 0.0;
        let mut losses = 0.0;

        for i in (candles.len() - period - 1)..(candles.len() - 1) {
            let change = candles[i + 1].close - candles[i].close;
            if change > 0.0 {
                gains += change;
            } else {
                losses -= change;
            }
        }

        let avg_gain = gains / period as f64;
        let avg_loss = losses / period as f64;

        if avg_loss == 0.0 {
            return 100.0;
        }

        let rs = avg_gain / avg_loss;
        100.0 - (100.0 / (1.0 + rs))
    }
}

impl Agent for AstrologerAgent {
    fn run(&mut self) {
        let portfolio_rx_handle = self.port_channel.clone();
        let ack_rx_handle = self.ack_channel.clone();
        let inventory_handle = self.inventory.clone();
        let cash_handle = self.cash.clone();
        let open_orders_handle_for_portfolio = self.open_orders.clone();
        let open_orders_handle_for_acks = self.open_orders.clone();
        let agent_id = self.id;

        // Spawn portfolio and ack listeners
        thread::spawn(move || {
            Self::run_portfolio_updater_internal(
                &portfolio_rx_handle,
                &inventory_handle,
                &cash_handle,
                &open_orders_handle_for_portfolio,
                agent_id,
            );
        });
        thread::spawn(move || {
            Self::run_ack_listener_internal(&ack_rx_handle, &open_orders_handle_for_acks);
        });

        // Main decision loop
        loop {
            thread::sleep(std::time::Duration::from_secs(5));
            self.decide_actions();
        }
    }

    fn decide_actions(&mut self) {
        let stock_ids: Vec<u64> = self
            .candle_data
            .iter()
            .map(|entry| entry.key().0)
            .collect::<std::collections::HashSet<_>>() // Collect into a HashSet to get unique IDs
            .into_iter()
            .collect(); // Convert back to a Vec

        let timeframe = TimeFrame::OneSecond;

        thread::scope(|s| {
            for stock_id in stock_ids {
                // Clone Arcs for the new thread
                let candle_data = self.candle_data.clone();
                let order_channel = self.order_channel.clone();
                let cash = self.cash.clone();
                let inventory = self.inventory.clone();
                let agent_id = self.id;

                s.spawn(move || {
                    let key = (stock_id, timeframe);
                    let candles_clone = match candle_data.get(&key) {
                        Some(candles_ref) => candles_ref.clone(),
                        None => return,
                    };

                    if candles_clone.len() < 50 {
                        return;
                    }

                    // --- SMA Crossover Strategy ---
                    let short_sma = Self::calculate_sma(&candles_clone, 10);
                    let long_sma = Self::calculate_sma(&candles_clone, 50);
                    let mut prev_candles = candles_clone.clone();
                    prev_candles.pop_back();
                    let prev_short_sma = Self::calculate_sma(&prev_candles, 10);
                    let prev_long_sma = Self::calculate_sma(&prev_candles, 50);

                    if prev_short_sma < prev_long_sma && short_sma > long_sma {
                        let cash_val = *cash.read().unwrap();
                        if let Some(price) = candles_clone.back().map(|c| c.close) {
                            let volume = ((cash_val * TRADE_SIZE_PERCENT) / price).floor() as u64;
                            if volume > 0 {
                                //println!("[Astrologer {}] Golden Cross on stock {}! BUYING {} shares.", agent_id, stock_id, volume);
                                order_channel
                                    .send(OrderRequest::MarketOrder {
                                        order_id: 0,
                                        agent_id,
                                        stock_id,
                                        side: Side::Buy,
                                        volume,
                                    })
                                    .ok();
                            }
                        }
                    }

                    if prev_short_sma > prev_long_sma && short_sma < long_sma {
                        let inv_val = *inventory.read().unwrap().get(&stock_id).unwrap_or(&0);
                        let volume = (inv_val.abs() as f64 * TRADE_SIZE_PERCENT).floor() as u64;
                        if volume > 0 {
                            //println!("[Astrologer {}] Death Cross on stock {}! SELLING {} shares.", agent_id, stock_id, volume);
                            order_channel
                                .send(OrderRequest::MarketOrder {
                                    order_id: 0,
                                    agent_id,
                                    stock_id,
                                    side: Side::Sell,
                                    volume,
                                })
                                .ok();
                        }
                    }

                    // --- RSI Strategy ---
                    let rsi = Self::calculate_rsi(&candles_clone, 14);
                    if rsi > 70.0 {
                        let inv_val = *inventory.read().unwrap().get(&stock_id).unwrap_or(&0);
                        let volume =
                            (inv_val.abs() as f64 * TRADE_SIZE_PERCENT / 2.0).floor() as u64; // Smaller size for RSI
                        if volume > 0 {
                            //println!("[Astrologer {}] Overbought on stock {} (RSI: {:.2})! SELLING {} shares.", agent_id, stock_id, rsi, volume);
                            order_channel
                                .send(OrderRequest::MarketOrder {
                                    order_id: 0,
                                    agent_id,
                                    stock_id,
                                    side: Side::Sell,
                                    volume,
                                })
                                .ok();
                        }
                    }

                    if rsi < 30.0 {
                        let cash_val = *cash.read().unwrap();
                        if let Some(price) = candles_clone.back().map(|c| c.close) {
                            let volume =
                                ((cash_val * TRADE_SIZE_PERCENT / 2.0) / price).floor() as u64; // Smaller size for RSI
                            if volume > 0 {
                                //println!("[Astrologer {}] Oversold on stock {} (RSI: {:.2})! BUYING {} shares.", agent_id, stock_id, rsi, volume);
                                order_channel
                                    .send(OrderRequest::MarketOrder {
                                        order_id: 0,
                                        agent_id,
                                        stock_id,
                                        side: Side::Buy,
                                        volume,
                                    })
                                    .ok();
                            }
                        }
                    }
                });
            }
        });
    }

    // --- Trait Methods ---
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

    fn acknowledge_order(&mut self) { /* Handled by listener */
    }
    fn update_portfolio(&mut self) { /* Handled by listener */
    }
    fn margin_call(&mut self) { /* Astrologer is too wise for margin calls */
    }

    fn evaluate_port(&mut self, market_view: &MarketState) -> f64 {
        let cash = *self.cash.read().unwrap();
        let inventory = self.inventory.read().unwrap();
        let mut port_value = cash;
        for (stock_id, &vol) in inventory.iter() {
            if let Some(px) = market_view.get_mid_price(*stock_id) {
                port_value += vol as f64 * (px as f64 / 100.0);
            }
        }
        port_value
    }

    fn get_pending_orders(&self) -> Vec<Order> {
        self.open_orders.read().unwrap().values().cloned().collect()
    }

    fn cancel_open_order(&mut self, order_id: u64) {
        if self
            .open_orders
            .write()
            .unwrap()
            .remove(&order_id)
            .is_some()
        {
            self.order_channel
                .send(OrderRequest::CancelOrder {
                    agent_id: self.id,
                    order_id,
                })
                .ok();
        }
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
}
