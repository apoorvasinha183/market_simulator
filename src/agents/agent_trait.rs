// src/agents/agent_trait.rs

use crate::simulation::orchestra::MarketState;
use crate::types::order::Order;

/// The core trait that all our participant types will implement.
pub trait Agent: Send + Sync {
    // === Core Decision-Making ===
    fn decide_actions(&mut self);

    // === High-Level API for RL / External Controllers ===
    fn buy_stock(&mut self, stock_id: u64, volume: u64);
    fn sell_stock(&mut self, stock_id: u64, volume: u64);

    // === Order & Position Management ===
    fn acknowledge_order(&mut self);
    fn margin_call(&mut self);
    fn update_portfolio(&mut self);
    fn evaluate_port(&mut self, market_view: &MarketState) -> f64;
    fn get_pending_orders(&self) -> Vec<Order>;
    fn cancel_open_order(&mut self, order_id: u64);
    fn run(&mut self);

    // === Getters & Housekeeping ===
    fn get_id(&self) -> usize;
    fn get_inventory(&self) -> i64;
    fn clone_agent(&self) -> Box<dyn Agent>;
}