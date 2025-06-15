// src/agents/agent_trait.rs

use crate::simulators::order_book::{OrderBook};
use crate::types::order::{Order, OrderRequest,Trade};
use crate::stocks::definitions::Symbol;
use std::collections::HashMap;
/// A read-only snapshot of the market given to an agent for decision-making.
pub struct MarketView<'a> {
    /// One book per ticker.
    pub order_books: &'a HashMap<Symbol, OrderBook>,
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
    fn update_portfolio(&mut self, trade_volume: i64, trade: &Trade);
    /// A way for the agent to look at the net worth of their portfolio
    fn evaluate_port(&mut self, market_view: &MarketView) -> f64;
    /// Get a list of all currently open orders for this agent.
    fn get_pending_orders(&self) -> Vec<Order>;

    /// Creates a request to cancel an open order.
    fn cancel_open_order(&mut self, order_id: u64) -> Vec<OrderRequest>;

    // === Getters & Housekeeping ===
    fn get_id(&self) -> usize;
    fn get_inventory(&self) -> i64;
    fn clone_agent(&self) -> Box<dyn Agent>;
}
/// The whale needs this
impl<'a> MarketView<'a> {
    /// Convenience: get the book for a specific symbol.
    pub fn book(&self, symbol: &Symbol) -> Option<&OrderBook> {
        self.order_books.get(symbol)
    }

    /// Mid-price helper (best bid + ask)/2 in cents.
    pub fn get_mid_price(&self, symbol: &Symbol) -> Option<u64> {
        let book = self.book(symbol)?;
        let best_bid = book.bids.keys().next_back()?;
        let best_ask = book.asks.keys().next()?;
        Some((best_bid + best_ask) / 2)
    }
}