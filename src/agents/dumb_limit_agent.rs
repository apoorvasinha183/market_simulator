// src/agents/dumb_limit_agent.rs

use super::agent_trait::{Agent, MarketView};
use super::config::{
    LIMIT_AGENT_ACTION_PROB,
    LIMIT_AGENT_MAX_OFFSET,
    LIMIT_AGENT_NUM_TRADERS, // Added NUM_TRADERS
    LIMIT_AGENT_VOL_MAX,
    LIMIT_AGENT_VOL_MIN,
    MARGIN_CALL_THRESHOLD,
};
//use super::latency;
use crate::agents::latency::LIMIT_AGENT_TICKS_UNTIL_ACTIVE;
use crate::simulators::order_book::Trade;
use crate::types::order::{Order, OrderRequest, Side};
use rand::Rng;
use std::collections::HashMap;

pub struct DumbLimitAgent {
    pub id: usize,
    inventory: i64,
    ticks_until_active: u32,
    open_orders: HashMap<u64, Order>,
}

impl DumbLimitAgent {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            inventory: 200_000_000,
            ticks_until_active: LIMIT_AGENT_TICKS_UNTIL_ACTIVE,
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
        let mut requests = Vec::new();

        // --- NEW: Ensemble Logic ---
        // Loop for each "trader" in our ensemble.
        for _ in 0..LIMIT_AGENT_NUM_TRADERS {
            if rng.gen_bool(LIMIT_AGENT_ACTION_PROB as f64) {
                let best_bid = market_view.order_book.bids.keys().last().copied();
                let best_ask = market_view.order_book.asks.keys().next().copied();

                if let (Some(bid), Some(ask)) = (best_bid, best_ask) {
                    if bid >= ask {
                        continue;
                    } // Skip if book is crossed

                    // Use the "dumber" logic for each individual trader
                    let side = if rng.gen_bool(0.5) {
                        Side::Buy
                    } else {
                        Side::Sell
                    };
                    let offset = rng.gen_range(1..=LIMIT_AGENT_MAX_OFFSET);
                    let price = match side {
                        Side::Buy => bid.saturating_add(offset),
                        Side::Sell => ask.saturating_sub(offset),
                    };

                    // Each trader places a small order
                    let volume = rng.gen_range(LIMIT_AGENT_VOL_MIN..=LIMIT_AGENT_VOL_MAX);

                    requests.push(OrderRequest::LimitOrder {
                        agent_id: self.id,
                        side,
                        price,
                        volume,
                    });
                }
            }
        }

        requests
    }

    // --- Fulfillment of the Agent Trait Contract ---

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
        if self.inventory <= MARGIN_CALL_THRESHOLD {
            let deficit = self.inventory.unsigned_abs();
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
            if let Some(ord) = self.open_orders.get_mut(&trade.maker_order_id) {
                ord.filled += trade.volume;
                if ord.filled >= ord.volume {
                    self.open_orders.remove(&trade.maker_order_id);
                }
            }
        }
    }

    fn get_pending_orders(&self) -> Vec<Order> {
        self.open_orders.values().cloned().collect()
    }

    fn cancel_open_order(&mut self, order_id: u64) -> Vec<OrderRequest> {
        if self.open_orders.remove(&order_id).is_some() {}
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
