// src/types/order.rs
//! Core order-flow types, now carrying `symbol` for multi-ticker support.
use serde::{Deserialize, Serialize};
/// Order side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Order {
    pub id: u64,
    pub agent_id: usize,
    pub stock_id: u64, // <── NEW
    pub side: Side,
    pub price: u64,
    pub volume: u64,
    pub filled: u64,
}

/// Message from an agent to the market engine.
#[derive(Debug, Serialize, Deserialize)]
pub enum OrderRequest {
    /// Limit order at a specific price.
    LimitOrder {
        agent_id: usize,
        stock_id: u64, // <── NEW
        side: Side,
        price: u64,
        volume: u64,
    },
    /// Market order that crosses the book immediately.
    MarketOrder {
        agent_id: usize,
        stock_id: u64, // <── NEW
        side: Side,
        volume: u64,
    },
    /// Cancel a previously placed order.
    CancelOrder {
        agent_id: usize, // to verify ownership
        order_id: u64,
    },
    // TODO: Add the ability to short the market, it I think allow naked shorts. A new Market Order type
}

/// Execution report emitted when two orders match.
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub price: u64,
    pub stock_id: u64,
    pub volume: u64,
    pub taker_agent_id: usize,
    pub maker_agent_id: usize,
    pub taker_side: Side,
    pub maker_order_id: u64,
}
// -----------------------------------------------------------------------------
//  Unit tests for order-flow types
// -----------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn side_opposite_is_involution() {
        assert_eq!(Side::Buy.opposite(), Side::Sell);
        assert_eq!(Side::Sell.opposite(), Side::Buy);
        // applying twice gets you back
        assert_eq!(Side::Buy.opposite().opposite(), Side::Buy);
        assert_eq!(Side::Sell.opposite().opposite(), Side::Sell);
    }

    #[test]
    fn order_struct_roundtrip_copy() {
        let o = Order {
            id: 1,
            agent_id: 7,
            stock_id: 2,
            side: Side::Buy,
            price: 10_500, // $105.00
            volume: 100,
            filled: 0,
        };
        // Because Order derives Copy we can duplicate without clone()
        let o2 = o;
        assert_eq!(o.id, o2.id);
        assert_eq!(o.side, o2.side);
    }

    #[test]
    fn limit_and_market_order_requests_hold_stock_id() {
        let limit = OrderRequest::LimitOrder {
            agent_id: 42,
            stock_id: 3,
            side: Side::Sell,
            price: 99_99,
            volume: 50,
        };
        let market = OrderRequest::MarketOrder {
            agent_id: 42,
            stock_id: 3,
            side: Side::Buy,
            volume: 75,
        };
        match limit {
            OrderRequest::LimitOrder { stock_id, .. } => assert_eq!(stock_id, 3),
            _ => panic!("expected limit order"),
        }
        match market {
            OrderRequest::MarketOrder { stock_id, .. } => assert_eq!(stock_id, 3),
            _ => panic!("expected market order"),
        }
    }

    #[test]
    fn trade_fields_consistent() {
        let t = Trade {
            price: 101_23,
            stock_id: 2,
            volume: 10,
            taker_agent_id: 5,
            maker_agent_id: 9,
            taker_side: Side::Buy,
            maker_order_id: 77,
        };
        assert_eq!(t.stock_id, 2);
        assert_eq!(t.price, 101_23);
        assert_eq!(t.taker_side.opposite(), Side::Sell);
    }
}
