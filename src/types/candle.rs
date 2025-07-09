
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TimeFrame {
    OneMinute,
    FiveMinutes,
    ThirtyMinutes,
}

impl TimeFrame {
    pub fn to_seconds(&self) -> u64 {
        match self {
            TimeFrame::OneMinute => 60,
            TimeFrame::FiveMinutes => 300,
            TimeFrame::ThirtyMinutes => 1800,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candle {
    pub stock_id: u64,
    pub timeframe: TimeFrame,
    pub timestamp: u64, // Unix timestamp for the start of the candle's period
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: u64,
}

impl Candle {
    // Method to update the candle with a new trade
    pub fn update(&mut self, price: f64, volume: u64) {
        self.high = self.high.max(price);
        self.low = self.low.min(price);
        self.close = price;
        self.volume += volume;
    }
}
