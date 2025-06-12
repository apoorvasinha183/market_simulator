// src/agents/ipo_agent.rs

use super::agent_trait::{Agent, MarketView};
use crate::types::order::{Order, OrderRequest, Side};
use std::collections::HashMap;

/// An agent that acts only once at the beginning of the simulation
/// to place the entire initial float of the asset on the market.
pub struct IpoAgent {
    pub id: usize,
    inventory: i64,
    has_acted: bool,
    /// Agent now statefully tracks its open orders.
    open_orders: HashMap<u64, Order>,
}

impl IpoAgent {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            // This agent is created holding the entire float of the company.
            inventory: 1_000_000,
            has_acted: false,
            open_orders: HashMap::new(),
        }
    }
}

impl Agent for IpoAgent {
    fn decide_actions(&mut self, _market_view: &MarketView) -> Vec<OrderRequest> {
        if self.has_acted {
            return vec![];
        }

        self.has_acted = true;
        println!("--- IPO AGENT IS ACTING ---");

        let mut orders = Vec::new();
        let num_price_levels = 20; // Increased levels for a smoother ladder
        let volume_per_level = (self.inventory / num_price_levels as i64) as u64;
        let start_price = 15000; // $150.00
        let tick_size = 5;       // $0.05 per tick

        for i in 0..num_price_levels {
            let price = start_price + i * tick_size;
            orders.push(OrderRequest::LimitOrder {
                agent_id: self.id,
                side: Side::Sell,
                price,
                volume: volume_per_level,
            });
        }
        orders
    }

    // --- Fulfillment of the new Agent trait contract ---

    fn buy_stock(&mut self, _volume: u64) -> Vec<OrderRequest> {
        // The IPO agent's job is to sell, not buy. This is a no-op.
        vec![]
    }

    fn sell_stock(&mut self, volume: u64) -> Vec<OrderRequest> {
        // This could be used for secondary offerings later, but for now,
        // we assume it only acts in decide_actions.
        // For a simple implementation, we can make it a no-op.
        vec![]
    }

    fn margin_call(&mut self) -> Vec<OrderRequest> {
        // This agent only starts with a long position, so it can't be margin called.
        vec![]
    }

    fn acknowledge_order(&mut self, order: Order) {
        self.open_orders.insert(order.id, order);
    }

    fn update_portfolio(&mut self, trade_volume: i64) {
        self.inventory += trade_volume;
        // A more advanced implementation would update the specific open orders
        // that were filled.
    }

    fn get_pending_orders(&self) -> Vec<Order> {
        self.open_orders.values().cloned().collect()
    }

    fn cancel_open_order(&mut self, order_id: u64) -> Vec<OrderRequest> {
        if self.open_orders.remove(&order_id).is_some() {
            println!("IpoAgent {} requesting cancel for order {}", self.id, order_id);
        }
        vec![]
    }

    fn get_id(&self) -> usize {
        self.id
    }

    fn get_inventory(&self) -> i64 {
        self.inventory
    }

    fn clone_agent(&self) -> Box<dyn Agent> {
        Box::new(IpoAgent::new(self.id))
    }
}
