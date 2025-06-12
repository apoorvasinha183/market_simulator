// src/agents/agent_trait.rs

use crate::simulators::order_book::OrderBook;
use crate::types::order::{Order, OrderRequest, Side};
/// A read-only snapshot of the market given to an agent for decision-making.
pub struct MarketView<'a> {
    pub order_book: &'a OrderBook,
}

/// The core trait that all our participant types will implement.
/// TODO: Later add a SYMBOL ticker argument when we are managing lots of stocks to handle 
pub trait Agent {
    // === Core Decision-Making ===
    fn decide_actions(&mut self, market_view: &MarketView) -> Vec<OrderRequest>;

    // === High-Level API for RL / External Controllers ===
    /// Creates a request to buy a certain volume of the asset.
    fn buy_stock(&mut self, volume: u64) -> Vec<OrderRequest>;
    /// Creates a request to sell a certain volume of the asset.
    fn sell_stock(&mut self, volume: u64) -> Vec<OrderRequest>;

    // === Order & Position Management ===
    /// The "promise fulfillment" callback from the Market.
    /// The Market calls this to give the agent the official Order object with its ID.
    fn acknowledge_order(&mut self, order: Order);

    /// The Market can call this to force an agent to cover a short position.
    fn margin_call(&mut self) -> Vec<OrderRequest>;

    /// A way for the simulation engine to update the agent's internal state after a trade.
    fn update_portfolio(&mut self, trade_volume: i64);
    
    /// Get a list of all currently open orders for this agent.
    fn get_pending_orders(&self) -> Vec<Order>;

    /// Creates a request to cancel an open order.
    fn cancel_open_order(&mut self, order_id: u64) -> Vec<OrderRequest>;

    // === Getters & Housekeeping ===
    fn get_id(&self) -> usize;
    fn get_inventory(&self) -> i64;
    fn clone_agent(&self) -> Box<dyn Agent>;
}
