// src/types/order.rs

/// The side of the market for an order.
#[derive(Debug, Clone, Copy)]
pub enum Side {
    Buy,
    Sell,
}

/// The set of commands an Agent can issue to the market.
/// This is our universal "order ticket".
#[derive(Debug)]
pub enum OrderRequest {
    /// An order to be placed at a specific price on the limit order book.
    LimitOrder {
        agent_id: usize,
        side: Side,
        price: u64, // Price in ticks
        volume: u64,
    },
    /// An order to be executed immediately at the best available price(s).
    MarketOrder {
        agent_id: usize,
        side: Side,
        volume: u64,
    },
    // We can add more later, like CancelOrder, etc.
}
