// src/agents/dumb_limit_agent.rs

use super::agent_trait::{Agent, MarketView};
use crate::types::order::{Order, OrderRequest, Side};
use rand::Rng;
use std::collections::HashMap;

pub struct DumbLimitAgent {
    pub id: usize,
    inventory: i64,
    action_probability: f64,
    ticks_until_active: u32,
    /// Agent now statefully tracks its own open orders.
    open_orders: HashMap<u64, Order>,
}

impl DumbLimitAgent {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            inventory: 2000000,
            action_probability: 0.2,
            ticks_until_active: 10,
            open_orders: HashMap::new(),
        }
    }
}

impl Agent for DumbLimitAgent {
    fn decide_actions(&mut self, market_view: &MarketView) -> Vec<OrderRequest> {
        if self.ticks_until_active > 0 {
            self.ticks_until_active -= 1;
            return vec![];
        }

        let mut rng = rand::thread_rng();
        if rng.gen_range(0.0..1.0) < self.action_probability {
            if let (Some(&bid), Some(&ask)) = (market_view.order_book.bids.keys().last(), market_view.order_book.asks.keys().next()) {
                let side = if rng.gen_bool(0.5) { Side::Buy } else { Side::Sell };
                let volume = rng.gen_range(50000..=200000);
                let price = match side {
                    Side::Buy => bid + 1,
                    Side::Sell => ask - 1,
                };
                if price > bid && price < ask {
                    return vec![OrderRequest::LimitOrder { agent_id: self.id, side, price, volume }];
                }
            }
        }
        
        // The short-covering logic has been moved to `margin_call`.
        vec![]
    }

    // --- Fulfillment of the new Agent trait contract ---

    fn buy_stock(&mut self, volume: u64) -> Vec<OrderRequest> {
        // For an RL agent, this would be a more complex decision.
        // For this simple agent, we'll just create a market order.
        vec![OrderRequest::MarketOrder { agent_id: self.id, side: Side::Buy, volume }]
    }

    fn sell_stock(&mut self, volume: u64) -> Vec<OrderRequest> {
        vec![OrderRequest::MarketOrder { agent_id: self.id, side: Side::Sell, volume }]
    }

    fn margin_call(&mut self) -> Vec<OrderRequest> {
        // Moved the short-covering logic here from decide_actions.
        if self.inventory <= -20000 {
            println!("!!! DumbLimitAgent {} MARGIN CALL! Covering short of {} !!!", self.id, self.inventory);
            let deficit = self.inventory.abs() as u64;
            return self.buy_stock(deficit);
        }
        vec![]
    }

    fn acknowledge_order(&mut self, order: Order) {
        self.open_orders.insert(order.id, order);
    }

    fn update_portfolio(&mut self, trade_volume: i64) {
        self.inventory += trade_volume;
        // A more advanced agent would update its open_orders map here
        // by checking which of its orders were filled in the trade.
        // For now, we just update the total inventory.
        println!("DumbLimitAgent {} new inventory: {}", self.id, self.inventory);
    }

    fn get_pending_orders(&self) -> Vec<Order> {
        self.open_orders.values().cloned().collect()
    }

    fn cancel_open_order(&mut self, order_id: u64) -> Vec<OrderRequest> {
        if self.open_orders.remove(&order_id).is_some() {
            println!("DumbLimitAgent {} requesting cancel for order {}", self.id, order_id);
            // This would return a real OrderRequest::Cancel in a full implementation.
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
        Box::new(DumbLimitAgent::new(self.id))
    }
}
