// src/types/order.rs

// --- ADD THIS DERIVE MACRO ---
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

#[derive(Debug, Clone, Copy)]
pub struct Order {
    pub id: u64,
    pub agent_id: usize,
    pub side: Side,
    pub price: u64,
    pub volume: u64,
    pub filled: u64,
}

#[derive(Debug)]
pub enum OrderRequest {
    LimitOrder {
        agent_id: usize,
        side: Side,
        price: u64,
        volume: u64,
    },
    MarketOrder {
        agent_id: usize,
        side: Side,
        volume: u64,
    },
    // A request to cancel a previously placed order.
    CancelOrder {
        agent_id: usize, // To verify ownership
        order_id: u64,
    },
}
