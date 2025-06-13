// src/agents/dumb_agent.rs

use super::agent_trait::{Agent, MarketView};
use super::config::{
    DUMB_AGENT_ACTION_PROB, DUMB_AGENT_LARGE_VOL_CHANCE, DUMB_AGENT_LARGE_VOL_MAX,
    DUMB_AGENT_LARGE_VOL_MIN, DUMB_AGENT_NUM_TRADERS, DUMB_AGENT_TYPICAL_VOL_MAX,
    DUMB_AGENT_TYPICAL_VOL_MIN,
};
//use super::latency;
use crate::agents::latency::DUMB_AGENT_TICKS_UNTIL_ACTIVE;
//use crate::market;
use crate::simulators::order_book::Trade;
use crate::types::order::{Order, OrderRequest, Side};
use rand::Rng;
use std::collections::HashMap;

pub struct DumbAgent {
    pub id: usize,
    inventory: i64,
    ticks_until_active: u32,
    open_orders: HashMap<u64, Order>,
    #[allow(dead_code)]
    margin:i128
}

impl DumbAgent {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            inventory: 300000000,
            ticks_until_active: DUMB_AGENT_TICKS_UNTIL_ACTIVE,
            open_orders: HashMap::new(),
            margin:1000000000
        }
    }
}

impl Agent for DumbAgent {
    fn decide_actions(&mut self, _market_view: &MarketView) -> Vec<OrderRequest> {
        if self.ticks_until_active > 0 {
            self.ticks_until_active -= 1;
            return vec![];
        }

        // --- Micro-Simulation Ensemble Logic using Central Config ---
        let mut rng = rand::thread_rng();
        let mut requests_this_tick = Vec::new();

        // Loop for each "trader" in our ensemble.
        for _ in 0..DUMB_AGENT_NUM_TRADERS {
            // Roll a dice to see if this individual acts.
            if rng.gen_range(0.0..1.0) < DUMB_AGENT_ACTION_PROB {
                let side = if rng.gen_bool(0.5) {
                    Side::Buy
                } else {
                    Side::Sell
                };

                // Determine volume using constants from the config file.
                let volume = if rng.gen_bool(DUMB_AGENT_LARGE_VOL_CHANCE) {
                    //println!("OMAIGAWD, This idiot yolod his lunch money");
                    rng.gen_range(DUMB_AGENT_LARGE_VOL_MIN..=DUMB_AGENT_LARGE_VOL_MAX)
                } else {
                    rng.gen_range(DUMB_AGENT_TYPICAL_VOL_MIN..=DUMB_AGENT_TYPICAL_VOL_MAX)
                };

                let request = if side == Side::Buy {
                    self.buy_stock(volume)
                } else {
                    self.sell_stock(volume)
                };
                requests_this_tick.extend(request);
            }
        }
        let liquidity = self.evaluate_port(_market_view);
        println!("Retail has a net position of {}",liquidity);
        requests_this_tick
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
        if self.inventory <= -2000 {
            let deficit = self.inventory.abs() as u64;
            return self.buy_stock(deficit);
        }
        vec![]
    }

    fn acknowledge_order(&mut self, order: Order) {
        self.open_orders.insert(order.id, order);
    }

    fn update_portfolio(&mut self, trade_volume: i64, trade: &Trade) {
        self.inventory += trade_volume;

        if trade.maker_agent_id == self.id {
            if let Some(order) = self.open_orders.get_mut(&trade.maker_order_id) {
                order.filled += trade.volume;
                if order.filled >= order.volume {
                    self.open_orders.remove(&trade.maker_order_id);
                }
            }
        }
    }

    fn get_pending_orders(&self) -> Vec<Order> {
        self.open_orders.values().cloned().collect()
    }

    fn cancel_open_order(&mut self, order_id: u64) -> Vec<OrderRequest> {
        if self.open_orders.contains_key(&order_id) {
            // A full implementation would return a real Cancel request.
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
    fn evaluate_port(&self,market_view: &MarketView) -> f64 {
        let price_cents = match market_view.get_mid_price() {
        Some(p) => p,
        None    => return 0.0,                // or whatever you deem appropriate
        };
        let value_cents = (self.inventory as i128)
        .checked_mul(price_cents as i128)
        .expect("portfolio value overflow");
        (value_cents as f64) / 100.0
    }
}
