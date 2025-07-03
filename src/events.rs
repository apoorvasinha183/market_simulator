// src/events.rs

use crate::types::Trade;

/// Defines the events that are broadcast across the central event bus.
#[derive(Debug, Clone)]
pub enum MarketEvent {
    /// A trade has been executed by the matching engine.
    TradeOccurred(Trade),

    /// The external sentiment score for a stock has been updated.
    SentimentUpdate { stock_id: u64, score: f64 },

    /// A periodic signal to mark the passage of time.
    Heartbeat,
}
