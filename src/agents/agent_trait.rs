// src/agents/agent_trait.rs

use crate::simulators::order_book::OrderBook;
use crate::types::order::OrderRequest;

/// A read-only snapshot of the market given to an agent for decision-making.
pub struct MarketView<'a> {
    pub order_book: &'a OrderBook,
}

/// The core trait that all our participant types will implement.
pub trait Agent {
    /// The "brain" of the agent. It observes the market and decides what to do.
    fn decide_actions(&mut self, market_view: &MarketView) -> Vec<OrderRequest>;

    /// A way for the simulation engine to update the agent's internal state after a trade.
    fn update_portfolio(&mut self, trade_volume: i64);

    /// A method to get the agent's unique ID.
    fn get_id(&self) -> usize;

    /// Creates a new, fresh instance of the agent for resetting the simulation.
    fn clone_agent(&self) -> Box<dyn Agent>;
}
