// src/types/order.rs

// --- ADD THIS DERIVE MACRO ---
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy)]
pub struct Order {
    pub id: u64,
    pub agent_id: usize,
    pub side: Side,
    pub price: u64,
    pub volume: u64,
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
}