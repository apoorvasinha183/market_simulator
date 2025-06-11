// src/agents/ipo_agent.rs

use super::agent_trait::{Agent, MarketView};
use crate::types::order::{OrderRequest, Side};

/// An agent that acts only once at the beginning of the simulation
/// to place the entire initial float of the asset on the market.
pub struct IpoAgent {
    pub id: usize,
    inventory: i64,
    has_acted: bool,
}

impl IpoAgent {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            // This agent is created holding the entire float of the company.
            inventory: 1_0000,
            has_acted: false,
        }
    }
}

impl Agent for IpoAgent {
    fn decide_actions(&mut self, _market_view: &MarketView) -> Vec<OrderRequest> {
        // This agent only acts once.
        if self.has_acted {
            return vec![];
        }

        // Set the flag immediately to ensure it never acts again.
        self.has_acted = true;

        println!("--- IPO AGENT IS ACTING ---");

        let mut orders = Vec::new();
        let num_price_levels = 20;
        let volume_per_level = (self.inventory / num_price_levels as i64) as u64;
        let start_price = 15000; // $150.00
        let tick_size = 5;       // $0.05 per tick

        // Create a sell wall to represent the IPO distribution.
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

    /// This agent still needs to track its inventory as its shares are sold off.
    fn update_portfolio(&mut self, trade_volume: i64) {
        self.inventory += trade_volume;
    }

    fn get_id(&self) -> usize {
        self.id
    }

    fn clone_agent(&self) -> Box<dyn Agent> {
        Box::new(IpoAgent::new(self.id))
    }
}
