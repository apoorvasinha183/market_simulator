// src/agents/market_maker_agent.rs

use super::agent_trait::{Agent, MarketView};
use crate::types::order::{OrderRequest, Side};
use rand::Rng;

pub struct MarketMakerAgent {
    pub id: usize,
    inventory: i64,
    desired_spread: u64,
    skew_factor: f64,
    initial_center_price: u64,
    ticks_until_active: u32,
}

impl MarketMakerAgent {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            inventory: 0,
            desired_spread: 25,
            skew_factor: 0.1,
            initial_center_price: 15000,
            ticks_until_active: 5,
        }
    }
}

impl Agent for MarketMakerAgent {
    fn decide_actions(&mut self, market_view: &MarketView) -> Vec<OrderRequest> {
        if self.ticks_until_active > 0 {
            self.ticks_until_active -= 1;
            return vec![];
        }

        let mut rng = rand::thread_rng();
        let best_bid = market_view.order_book.bids.keys().last().cloned();
        let best_ask = market_view.order_book.asks.keys().next().cloned();

        // --- NEW, MORE ROBUST LOGIC ---
        // Determine the center price based on the state of the book.
        let center_price = match (best_bid, best_ask) {
            // Case 1: A normal, two-sided market exists.
            (Some(bid), Some(ask)) => {
                if ask > bid { (bid + ask) / 2 } else { return vec![]; } // Don't act on crossed book
            },
            // Case 2: Only asks exist (like after the IPO). Use the best ask as a reference.
            (None, Some(ask)) => ask - self.desired_spread,
            // Case 3: Only bids exist. Use the best bid as a reference.
            (Some(bid), None) => bid + self.desired_spread,
            // Case 4: The book is completely empty. Do nothing and wait.
            (None, None) => return vec![],
        };

        // The skew logic remains the same, but it's now applied to our calculated center_price.
        let inventory_skew = (self.inventory as f64 * self.skew_factor) as i64;
        let our_center_price = (center_price as i64 - inventory_skew) as u64;

        let our_bid = our_center_price.saturating_sub(self.desired_spread / 2);
        let our_ask = our_center_price.saturating_add(self.desired_spread / 2);

        // Safety check to ensure our quotes are valid and don't cross the existing market (if any).
        if our_ask > our_bid {
             if best_ask.map_or(true, |ask| our_bid < ask) && best_bid.map_or(true, |bid| our_ask > bid) {
                let volume = rng.gen_range(100..=300);
                return vec![
                    OrderRequest::LimitOrder { agent_id: self.id, side: Side::Buy, price: our_bid, volume },
                    OrderRequest::LimitOrder { agent_id: self.id, side: Side::Sell, price: our_ask, volume },
                ];
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
        Box::new(MarketMakerAgent::new(self.id))
    }
}
