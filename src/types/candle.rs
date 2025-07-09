use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TimeFrame {
    HundredMillis,
    OneSecond,
    TenSeconds,
    OneMinute,
    FiveMinutes,
    ThirtyMinutes,
}

impl fmt::Display for TimeFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TimeFrame::HundredMillis => write!(f, "100ms"),
            TimeFrame::OneSecond => write!(f, "1s"),
            TimeFrame::TenSeconds => write!(f, "10s"),
            TimeFrame::OneMinute => write!(f, "1m"),
            TimeFrame::FiveMinutes => write!(f, "5m"),
            TimeFrame::ThirtyMinutes => write!(f, "30m"),
        }
    }
}

impl TimeFrame {
    pub fn to_millis(&self) -> u64 {
        match self {
            TimeFrame::HundredMillis => 100,
            TimeFrame::OneSecond => 1_000,
            TimeFrame::TenSeconds => 10_000,
            TimeFrame::OneMinute => 60_000,
            TimeFrame::FiveMinutes => 300_000,
            TimeFrame::ThirtyMinutes => 1_800_000,
        }
    }

    pub fn all() -> Vec<TimeFrame> {
        vec![
            TimeFrame::HundredMillis,
            TimeFrame::OneSecond,
            TimeFrame::TenSeconds,
            TimeFrame::OneMinute,
            TimeFrame::FiveMinutes,
            TimeFrame::ThirtyMinutes,
        ]
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
