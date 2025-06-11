// src/agents/dumb_limit_agent.rs

use super::agent_trait::{Agent, MarketView};
use crate::types::order::{OrderRequest, Side};
use rand::Rng;

pub struct DumbLimitAgent {
    pub id: usize,
    pub inventory: i64,
    action_probability: f64,
}

impl DumbLimitAgent {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            inventory: 0,
            action_probability: 0.2,
        }
    }
}

impl Agent for DumbLimitAgent {
    fn decide_actions(&mut self, market_view: &MarketView) -> Vec<OrderRequest> {
        let mut rng = rand::thread_rng();
        if rng.gen_range(0.0..1.0) < self.action_probability {
            if let (Some(&bid), Some(&ask)) = (market_view.order_book.bids.keys().last(), market_view.order_book.asks.keys().next()) {
                let side = if rng.gen_bool(0.5) { Side::Buy } else { Side::Sell };
                let volume = rng.gen_range(50..=200);
                let price = match side {
                    Side::Buy => bid + 1,
                    Side::Sell => ask - 1,
                };
                if price > bid && price < ask { // Ensure we don't cross the spread
                    return vec![OrderRequest::LimitOrder { agent_id: self.id, side, price, volume }];
                }
            }
        }
        vec![]
    }

    fn update_portfolio(&mut self, trade_volume: i64) {
        self.inventory += trade_volume;
    }

    fn get_id(&self) -> usize {
        self.id
    }

    fn clone_agent(&self) -> Box<dyn Agent> {
        Box::new(DumbLimitAgent::new(self.id))
    }
}
