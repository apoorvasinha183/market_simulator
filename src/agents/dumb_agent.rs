// src/agents/dumb_agent.rs

use super::agent_trait::{Agent, MarketView};
use crate::types::order::{OrderRequest, Side};
use rand::Rng;

pub struct DumbAgent {
    pub id: usize,
    pub inventory: i64,
    action_probability: f64,
}

impl DumbAgent {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            inventory: 0,
            action_probability: 0.1,
        }
    }
}

impl Agent for DumbAgent {
    fn decide_actions(&mut self, _market_view: &MarketView) -> Vec<OrderRequest> {
        let mut rng = rand::thread_rng();
        if rng.gen_range(0.0..1.0) < self.action_probability {
            let side = if rng.gen_bool(0.5) { Side::Buy } else { Side::Sell };
            let volume = rng.gen_range(100..=500);
            vec![OrderRequest::MarketOrder { agent_id: self.id, side, volume }]
        } else {
            vec![]
        }
    }

    fn update_portfolio(&mut self, trade_volume: i64) {
        self.inventory += trade_volume;
    }

    fn get_id(&self) -> usize {
        self.id
    }

    fn clone_agent(&self) -> Box<dyn Agent> {
        // Create a new instance with the same ID but reset inventory.
        Box::new(DumbAgent::new(self.id))
    }
}
