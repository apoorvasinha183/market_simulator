// src/agents/dumb_limit_agent.rs

use super::agent_trait::{Agent, MarketView};
use crate::types::order::{OrderRequest, Side};
use rand::Rng;

pub struct DumbLimitAgent {
    pub id: usize,
    /// --- NOW IN USE: This agent now tracks its inventory ---
    inventory: i64,
    action_probability: f64,
    ticks_until_active: u32,
    
}

impl DumbLimitAgent {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            inventory: 0, // Start with a flat inventory
            action_probability: 0.2,
            ticks_until_active: 10,
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
                let volume = rng.gen_range(50..=200);
                let price = match side {
                    Side::Buy => bid + 1,
                    Side::Sell => ask - 1,
                };
                if price > bid && price < ask {
                    return vec![OrderRequest::LimitOrder { agent_id: self.id, side, price, volume }];
                }
            }
        }
        // short covering
        if (self.inventory <= -200){
            let deficit = -1*self.inventory;
            let absolute_deficit = deficit as u64;
            return vec![OrderRequest::MarketOrder { agent_id: self.id,side: Side::Buy,volume:absolute_deficit }];
        }
        vec![]
    }

    /// --- IMPLEMENTED: This function now correctly updates our state ---
    fn update_portfolio(&mut self, trade_volume: i64) {
        self.inventory += trade_volume;
        
            // cover them with a 
        // For debugging, we can print the new inventory.
        // You can comment this out later to reduce console noise.
        //println!("DumbLimitAgent {} new inventory: {}", self.id, self.inventory);
    }

    fn get_id(&self) -> usize {
        self.id
    }

    fn clone_agent(&self) -> Box<dyn Agent> {
        Box::new(DumbLimitAgent::new(self.id))
    }
}
