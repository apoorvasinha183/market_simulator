// src/types/order.rs
//! Core order-flow types, now carrying `symbol` for multi-ticker support.

use crate::stocks::definitions::Symbol;

/// Order side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Buy,
    Sell,
}

impl Side {
    /// Returns the opposite side of the market.
    pub fn opposite(self) -> Self {
        match self {
            Side::Buy => Side::Sell,
            Side::Sell => Side::Buy,
        }
    }
}

/// Order currently resting in an order book.
#[derive(Debug, Clone)]
pub struct Order {
    pub id:        u64,
    pub agent_id:  usize,
    pub symbol:    Symbol,   // <── NEW
    pub side:      Side,
    pub price:     u64,
    pub volume:    u64,
    pub filled:    u64,
}

/// Message from an agent to the market engine.
#[derive(Debug)]
pub enum OrderRequest {
    /// Limit order at a specific price.
    LimitOrder {
        agent_id: usize,
        symbol:   Symbol,    // <── NEW
        side:     Side,
        price:    u64,
        volume:   u64,
    },
    /// Market order that crosses the book immediately.
    MarketOrder {
        agent_id: usize,
        symbol:   Symbol,    // <── NEW
        side:     Side,
        volume:   u64,
    },
    /// Cancel a previously placed order.
    CancelOrder {
        agent_id: usize, // to verify ownership
        order_id: u64,
    },
}

/// Execution report emitted when two orders match.
#[derive(Debug, Clone)]
pub struct Trade {
    pub price: u64,
    pub symbol:   Symbol,
    pub volume: u64,
    pub taker_agent_id: usize,
    pub maker_agent_id: usize,
    pub taker_side: Side,
    pub maker_order_id: u64,
}
