// src/agents/astrologer_agent.rs

use crossbeam_channel::{Receiver, Sender};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;

use crate::agents::agent_trait::Agent;
use crate::simulation::candle_analyzer::CandleDataHandle;
use crate::simulation::orchestra::MarketState;
use crate::types::candle::{Candle, TimeFrame};
use crate::types::order::{Order, OrderRequest, Trade};

/// An agent that makes trading decisions based on technical analysis of candlestick data.
#[derive(Clone)]
pub struct AstrologerAgent {
    id: usize,
    order_channel: Sender<OrderRequest>,
    #[allow(dead_code)]
    ack_channel: Arc<Mutex<Receiver<Order>>>,
    #[allow(dead_code)]
    port_channel: Arc<Mutex<Receiver<Trade>>>,
    candle_data: CandleDataHandle,
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
            ack_channel: Arc::new(Mutex::new(ack_channel)),
            port_channel: Arc::new(Mutex::new(port_channel)),
            candle_data,
        }
    }

    /// Calculates the Simple Moving Average (SMA) for a given period.
    fn calculate_sma(candles: &VecDeque<Candle>, period: usize) -> f64 {
        if candles.len() < period {
            return 0.0;
        }
        let sum: f64 = candles.iter().rev().take(period).map(|c| c.close).sum();
        sum / period as f64
    }
}

impl Agent for AstrologerAgent {
    fn run(&mut self) {
        // The Astrologer ponders the charts every 5 seconds.
        loop {
            thread::sleep(std::time::Duration::from_secs(5));
            self.decide_actions();
        }
    }

    fn decide_actions(&mut self) {
        // This Astrologer is obsessed with stock 1 on the 1-minute chart.
        let stock_to_analyze = 1;
        let timeframe = TimeFrame::OneMinute;
        let key = (stock_to_analyze, timeframe);

        // Get a clone of the candles to avoid borrowing issues
        let candles_clone = if let Some(candles_ref) = self.candle_data.get(&key) {
            candles_ref.clone()
        } else {
            return; // No candles yet for this stock/timeframe
        };

        // We need at least 50 candles to trust the stars (long SMA period).
        if candles_clone.len() < 50 {
            return;
        }

        // --- Simple Moving Average (SMA) Crossover Strategy ---
        let short_sma = Self::calculate_sma(&candles_clone, 10);
        let long_sma = Self::calculate_sma(&candles_clone, 50);

        // To detect a crossover, we need the previous state.
        let mut prev_candles = candles_clone.clone();
        prev_candles.pop_back(); // Remove the most recent candle to get the previous state

        let prev_short_sma = Self::calculate_sma(&prev_candles, 10);
        let prev_long_sma = Self::calculate_sma(&prev_candles, 50);

        // A "Golden Cross" occurs when the short-term average crosses ABOVE the long-term average.
        // This is a classic bullish signal.
        if prev_short_sma < prev_long_sma && short_sma > long_sma {
            println!(
                "[Astrologer {}] The stars align for stock {}! A Golden Cross! I must BUY!",
                self.id, stock_to_analyze
            );
            self.buy_stock(stock_to_analyze, 100); // Buy 100 shares
        }

        // A "Death Cross" occurs when the short-term average crosses BELOW the long-term average.
        // This is a classic bearish signal.
        if prev_short_sma > prev_long_sma && short_sma < long_sma {
            println!(
                "[Astrologer {}] The omens are dark for stock {}! A Death Cross! I must SELL!",
                self.id, stock_to_analyze
            );
            self.sell_stock(stock_to_analyze, 100); // Sell 100 shares
        }
    }

    fn buy_stock(&mut self, stock_id: u64, volume: u64) {
        let req = OrderRequest::MarketOrder {
            order_id: 0, // Will be filled by the market
            agent_id: self.id,
            stock_id,
            side: crate::types::order::Side::Buy,
            volume,
        };
        self.order_channel.send(req).ok();
    }

    fn sell_stock(&mut self, stock_id: u64, volume: u64) {
        let req = OrderRequest::MarketOrder {
            order_id: 0, // Will be filled by the market
            agent_id: self.id,
            stock_id,
            side: crate::types::order::Side::Sell,
            volume,
        };
        self.order_channel.send(req).ok();
    }

    // --- Other required trait methods (mostly stubs for this agent) ---
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
