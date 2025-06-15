// src/agents/agent_trait.rs

// FIXED: Use the top-level re-exported types, which is cleaner and more robust.
use crate::{MarketView, Order, OrderRequest, Trade};
use std::collections::HashMap;
use crate::stocks::definitions::Symbol;

pub trait Agent {
    fn decide_actions(&mut self, market_view: &MarketView) -> Vec<OrderRequest>;
    fn buy_stock(&mut self, volume: u64, symbol: &Symbol) -> Vec<OrderRequest>;
    fn sell_stock(&mut self, volume: u64, symbol: &Symbol) -> Vec<OrderRequest>;
    fn margin_call(&mut self) -> Vec<OrderRequest>;
    fn acknowledge_order(&mut self, order: Order);
    fn update_portfolio(&mut self, trade_volume: i64, trade: &Trade);
    fn get_pending_orders(&self) -> Vec<Order>;
    fn cancel_open_order(&mut self, order_id: u64) -> Vec<OrderRequest>;
    fn get_id(&self) -> usize;
    fn get_inventory(&self) -> &HashMap<Symbol, i64>;
    fn clone_agent(&self) -> Box<dyn Agent>;
    fn evaluate_port(&mut self, market_view: &MarketView) -> f64;
}

// NOTE: The impl MarketView block from the user's paste was incorrect,
// as the struct is not defined in this file. It belongs in market_trait.rs,
// where it has been made public.