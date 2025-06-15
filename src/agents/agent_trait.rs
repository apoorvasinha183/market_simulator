// src/agents/agent_trait.rs

// NEW: We need these types for the refactor.
use crate::stocks::definitions::Symbol;
use crate::simulators::order_book::{OrderBook, Trade};
use crate::types::order::{Order, OrderRequest};
use std::collections::HashMap;

/// A read-only snapshot of the market given to an agent for decision-making.
// CHANGED: The view now provides access to all order books and prices.
pub struct MarketView<'a> {
    pub order_books: &'a HashMap<Symbol, OrderBook>,
    pub last_traded_prices: &'a HashMap<Symbol, f64>,
}

/// The core trait that all our participant types will implement.
pub trait Agent {
    // === Core Decision-Making ===
    fn decide_actions(&mut self, market_view: &MarketView) -> Vec<OrderRequest>;

    // === High-Level API for RL / External Controllers ===
    // CHANGED: Must specify which symbol to buy/sell.
    fn buy_stock(&mut self, volume: u64, symbol: &Symbol) -> Vec<OrderRequest>;
    fn sell_stock(&mut self, volume: u64, symbol: &Symbol) -> Vec<OrderRequest>;

    // === Order & Position Management ===
    fn acknowledge_order(&mut self, order: Order);
    fn margin_call(&mut self) -> Vec<OrderRequest>;
    fn update_portfolio(&mut self, trade_volume: i64, trade: &Trade);
    fn get_pending_orders(&self) -> Vec<Order>;
    fn cancel_open_order(&mut self, order_id: u64) -> Vec<OrderRequest>;

    // === Getters & Housekeeping ===
    fn get_id(&self) -> usize;
    // CHANGED: Inventory is now per-symbol.
    fn get_inventory(&self) -> &HashMap<Symbol, i64>;
    fn clone_agent(&self) -> Box<dyn Agent>;
    fn evaluate_port(&mut self, market_view: &MarketView) -> f64;
}

// CHANGED: The helper function now lives on MarketView and requires a symbol.
impl<'a> MarketView<'a> {
    /// Calculates the mid-price for a specific symbol if a valid spread exists.
    pub fn get_mid_price(&self, symbol: &Symbol) -> Option<u64> {
        // First, get the correct order book for the requested symbol.
        if let Some(order_book) = self.order_books.get(symbol) {
            let best_bid = order_book.bids.keys().last();
            let best_ask = order_book.asks.keys().next();

            if let (Some(bid), Some(ask)) = (best_bid, best_ask) {
                if ask > bid {
                    Some((bid + ask) / 2)
                } else {
                    None // Book is crossed
                }
            } else {
                None // One or both sides are empty
            }
        } else {
            None // The symbol does not have an order book in this market
        }
    }
}
