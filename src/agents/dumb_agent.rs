// src/agents/dumb_agent.rs

use super::agent_trait::{Agent, MarketView};
use crate::types::order::{Order, OrderRequest, Side};
use rand::Rng;
use std::collections::HashMap;

pub struct DumbAgent {
    pub id: usize,
    inventory: i64,
    action_probability: f64,
    ticks_until_active: u32,
    /// Agent now statefully tracks its own open orders, mapping order_id -> Order.
    open_orders: HashMap<u64, Order>,
}

impl DumbAgent {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            inventory: 3000000,
            action_probability: 0.1,
            ticks_until_active: 10,
            open_orders: HashMap::new(),
        }
    }
}

impl Agent for DumbAgent {
    fn decide_actions(&mut self, _market_view: &MarketView) -> Vec<OrderRequest> {
        if self.ticks_until_active > 0 {
            self.ticks_until_active -= 1;
            return vec![];
        }

        let mut rng = rand::thread_rng();
        if rng.gen_range(0.0..1.0) < self.action_probability {
            let volume = if rng.gen_bool(0.15) {
                rng.gen_range(75000..=100000)
            } else {
                rng.gen_range(10000..=50000)
            };

            if rng.gen_bool(0.5) {
                return self.buy_stock(volume);
            } else {
                return self.sell_stock(volume);
            }
        }
        vec![]
    }

    fn buy_stock(&mut self, volume: u64) -> Vec<OrderRequest> {
        vec![OrderRequest::MarketOrder {
            agent_id: self.id,
            side: Side::Buy,
            volume,
        }]
    }

    fn sell_stock(&mut self, volume: u64) -> Vec<OrderRequest> {
        vec![OrderRequest::MarketOrder {
            agent_id: self.id,
            side: Side::Sell,
            volume,
        }]
    }

    fn margin_call(&mut self) -> Vec<OrderRequest> {
        if self.inventory <= -20000 {
            println!(
                "!!! DumbAgent {} MARGIN CALL! Covering short of {} !!!",
                self.id, self.inventory
            );
            let deficit = self.inventory.abs() as u64;
            return self.buy_stock(deficit);
        }
        vec![]
    }
    
    fn acknowledge_order(&mut self, order: Order) {
        // A limit order agent would use this to track open orders.
        // For this agent, it's less critical but good for logging.
        self.open_orders.insert(order.id, order);
    }

    fn update_portfolio(&mut self, trade_volume: i64) {
        self.inventory += trade_volume;
        // The logic for removing filled orders would go here.
        // For market orders, we can assume they are filled instantly.
        // For limit orders, we'd need to check the remaining volume.
        println!("DumbAgent {} new inventory: {}", self.id, self.inventory);
    }
    
    fn get_pending_orders(&self) -> Vec<Order> {
        self.open_orders.values().cloned().collect()
    }

    fn cancel_open_order(&mut self, order_id: u64) -> Vec<OrderRequest> {
        if self.open_orders.remove(&order_id).is_some() {
            println!(
                "DumbAgent {} is requesting to cancel order {}",
                self.id, order_id
            );
            // In a full implementation, this would return a Cancel request.
            // e.g., vec![OrderRequest::Cancel { order_id }]
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
        Box::new(DumbAgent::new(self.id))
    }
}
