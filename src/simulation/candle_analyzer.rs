
use crate::types::candle::{Candle, TimeFrame};
use crate::types::order::Trade;
use dashmap::DashMap;
use std::collections::VecDeque;
use std::sync::Arc;
use crossbeam_channel::Receiver;

// The shared, read-only handle for agents
pub type CandleDataHandle = Arc<DashMap<(u64, TimeFrame), VecDeque<Candle>>>;

pub struct CandleAnalyzer {
    trade_receiver: Receiver<Trade>,
    candle_data: CandleDataHandle,
    // Tracks the currently "open" candle for each stock/timeframe
    active_candles: DashMap<(u64, TimeFrame), Candle>,
}

impl CandleAnalyzer {
    pub fn new(trade_receiver: Receiver<Trade>) -> (Self, CandleDataHandle) {
        let candle_data = Arc::new(DashMap::new());
        let analyzer = Self {
            trade_receiver,
            candle_data: candle_data.clone(),
            active_candles: DashMap::new(),
        };
        (analyzer, candle_data)
    }

    pub fn run(self) {
        // The main loop that processes trades
        while let Ok(trade) = self.trade_receiver.recv() {
            // For each timeframe we care about...
            for &timeframe in &[TimeFrame::OneMinute, TimeFrame::FiveMinutes, TimeFrame::ThirtyMinutes] {
                self.process_trade_for_timeframe(&trade, timeframe);
            }
        }
    }

    fn process_trade_for_timeframe(&self, trade: &Trade, timeframe: TimeFrame) {
        let timeframe_seconds = timeframe.to_seconds();
        // Ensure we have a valid timestamp for the trade itself
        let trade_timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let candle_timestamp = (trade_timestamp / timeframe_seconds) * timeframe_seconds;

        let key = (trade.stock_id, timeframe);

        // Check if the trade belongs to the currently active candle
        if let Some(mut active_candle) = self.active_candles.get_mut(&key) {
            if active_candle.timestamp == candle_timestamp {
                // It does, so just update it
                active_candle.update(trade.price as f64 / 100.0, trade.volume);
                return;
            } else {
                // It's a new period. Finalize the old candle and create a new one.
                let finished_candle = active_candle.clone();
                let mut history = self.candle_data.entry(key).or_default();
                history.push_back(finished_candle);
                if history.len() > 500 { // Keep history trimmed
                    history.pop_front();
                }
            }
        }

        // Create a brand new candle
        let new_candle = Candle {
            stock_id: trade.stock_id,
            timeframe,
            timestamp: candle_timestamp,
            open: trade.price as f64 / 100.0,
            high: trade.price as f64 / 100.0,
            low: trade.price as f64 / 100.0,
            close: trade.price as f64 / 100.0,
            volume: trade.volume,
        };
        self.active_candles.insert(key, new_candle);
    }
}
