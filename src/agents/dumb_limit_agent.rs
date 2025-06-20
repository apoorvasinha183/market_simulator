// src/agents/dumb_limit_agent.rs
use rand::{Rng, seq::SliceRandom};
use std::collections::HashMap;

use super::{
    agent_trait::{Agent, MarketView},
    config::{
        LIMIT_AGENT_ACTION_PROB, LIMIT_AGENT_MAX_OFFSET, LIMIT_AGENT_NUM_TRADERS,
        LIMIT_AGENT_VOL_MAX, LIMIT_AGENT_VOL_MIN, MARGIN_CALL_THRESHOLD,
    },
};
use crate::{
    agents::latency::LIMIT_AGENT_TICKS_UNTIL_ACTIVE,
    types::order::{Order, OrderRequest, Side, Trade},
};

pub struct DumbLimitAgent {
    id: usize,
    inventory: i64,
    ticks_until_active: u32,
    open_orders: HashMap<u64, Order>,
    cash: f64,
    // allow dead code
    #[allow(dead_code)]
    margin: f64,
    port_value: f64,
}

impl DumbLimitAgent {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            inventory: 200_000_000,
            ticks_until_active: LIMIT_AGENT_TICKS_UNTIL_ACTIVE,
            open_orders: HashMap::new(),
            cash: 100_000_000.0,
            margin: 10_000_000_000.0,
            port_value: 0.0,
        }
    }
}

// -----------------------------------------------------------------------------
//  Agent impl
// -----------------------------------------------------------------------------
impl Agent for DumbLimitAgent {
    fn decide_actions(&mut self, view: &MarketView) -> Vec<OrderRequest> {
        if self.ticks_until_active > 0 {
            self.ticks_until_active -= 1;
            return vec![];
        }

        let mut rng = rand::thread_rng();
        let mut out = Vec::new();

        /* choose a random instrument for this tick */
        let ids: Vec<u64> = view.stocks.get_all_ids();
        if ids.is_empty() {
            return out;
        }
        let stock_id = *ids.choose(&mut rng).unwrap();
        let book = match view.book(stock_id) {
            Some(b) => b,
            None => return out,
        };

        let best_bid = book.bids.keys().next_back().copied();
        let best_ask = book.asks.keys().next().copied();

        for _ in 0..LIMIT_AGENT_NUM_TRADERS {
            if !rng.gen_bool(LIMIT_AGENT_ACTION_PROB) {
                continue;
            }

            if let (Some(bid), Some(ask)) = (best_bid, best_ask) {
                if bid >= ask {
                    continue; // crossed book
                }

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
                let volume = rng.gen_range(LIMIT_AGENT_VOL_MIN..=LIMIT_AGENT_VOL_MAX);

                out.push(OrderRequest::LimitOrder {
                    agent_id: self.id,
                    stock_id,
                    side,
                    price,
                    volume,
                });
            }
        }
        out
    }
    fn run(&mut self) {}
    /* market-order helpers -------------------------------------------------- */
    fn buy_stock(&mut self, stock_id: u64, volume: u64) -> Vec<OrderRequest> {
        vec![OrderRequest::MarketOrder {
            agent_id: self.id,
            stock_id,
            side: Side::Buy,
            volume,
        }]
    }
    fn sell_stock(&mut self, stock_id: u64, volume: u64) -> Vec<OrderRequest> {
        vec![OrderRequest::MarketOrder {
            agent_id: self.id,
            stock_id,
            side: Side::Sell,
            volume,
        }]
    }

    /* margin call: liquidate when inventory too low ------------------------ */
    fn margin_call(&mut self) -> Vec<OrderRequest> {
        if self.inventory <= MARGIN_CALL_THRESHOLD {
            let deficit = self.inventory.unsigned_abs();
            /* choose any stock_id we’re active in (else 0) */
            let sid = self
                .open_orders
                .values()
                .next()
                .map(|o| o.stock_id)
                .unwrap_or(0);
            return self.buy_stock(sid, deficit);
        }
        vec![]
    }

    /* bookkeeping ----------------------------------------------------------- */
    fn acknowledge_order(&mut self, o: Order) {
        self.open_orders.insert(o.id, o);
    }

    fn update_portfolio(&mut self, vol: i64, tr: &Trade) {
        self.inventory += vol;
        self.cash -= vol as f64 * (tr.price as f64 / 100.0);

        if tr.maker_agent_id == self.id {
            if let Some(o) = self.open_orders.get_mut(&tr.maker_order_id) {
                o.filled += tr.volume;
                if o.filled >= o.volume {
                    self.open_orders.remove(&tr.maker_order_id);
                }
            }
        }
    }

    fn get_pending_orders(&self) -> Vec<Order> {
        self.open_orders.values().cloned().collect()
    }
    fn cancel_open_order(&mut self, id: u64) -> Vec<OrderRequest> {
        self.open_orders.remove(&id);
        vec![]
    }

    /* misc getters ---------------------------------------------------------- */
    fn get_id(&self) -> usize {
        self.id
    }
    fn get_inventory(&self) -> i64 {
        self.inventory
    }
    fn clone_agent(&self) -> Box<dyn Agent> {
        Box::new(DumbLimitAgent::new(self.id))
    }

    fn evaluate_port(&mut self, view: &MarketView) -> f64 {
        let sid = *view.stocks.get_all_ids().first().unwrap_or(&0);
        if let Some(px) = view.get_mid_price(sid) {
            self.port_value = self.inventory as f64 * (px as f64 / 100.0);
        }
        self.port_value
    }
}
