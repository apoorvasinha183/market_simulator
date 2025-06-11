// src/agents/dumb_agent.rs

use super::agent_trait::{Agent, MarketView};
use crate::types::order::{OrderRequest, Side};
use rand::Rng;

pub struct DumbAgent {
    pub id: usize,
    /// --- NOW IN USE: This agent now tracks its inventory ---
    inventory: i64,
    action_probability: f64,
    ticks_until_active: u32,
}

impl DumbAgent {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            inventory: 3000000, // Start with a flat inventory
            action_probability: 0.1,
            ticks_until_active: 10,
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
            let side = if rng.gen_bool(0.5) { Side::Buy } else { Side::Sell };

            let volume = if rng.gen_bool(0.15) {
                rng.gen_range(75000..=100000)
            } else {
                rng.gen_range(10000..=50000)
            };
            
            vec![OrderRequest::MarketOrder { agent_id: self.id, side, volume }]
        } else {
            // short covering
        if (self.inventory <= -20000){
            let deficit = -1*self.inventory;
            let absolute_deficit = deficit as u64;
            return vec![OrderRequest::MarketOrder { agent_id: self.id,side: Side::Buy,volume:absolute_deficit }];
        }
            vec![]
        }
    }

    /// --- IMPLEMENTED: This function now correctly updates our state ---
    fn update_portfolio(&mut self, trade_volume: i64) {
        self.inventory += trade_volume;
        // For debugging, we can print the new inventory.
        // You can comment this out later to reduce console noise.
        println!("DumbAgent {} new inventory: {}", self.id, self.inventory);
    }

    fn get_id(&self) -> usize {
        self.id
    }

    fn clone_agent(&self) -> Box<dyn Agent> {
        Box::new(DumbAgent::new(self.id))
    }
}
