// src/agents/ipo_agent.rs
use std::collections::HashMap;

use super::agent_trait::{Agent, MarketView};
use crate::types::order::{Order, OrderRequest, Side, Trade};

/// IPO agent: posts one ladder of sell limits at boot and is done.
pub struct IpoAgent {
    id: usize,
    inventory: i64,
    has_acted: bool,
    open_orders: HashMap<u64, Order>,
}

impl IpoAgent {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            inventory: 1_000_000, // float to distribute
            has_acted: false,
            open_orders: HashMap::new(),
        }
    }
}

// -----------------------------------------------------------------------------
//  Agent impl
// -----------------------------------------------------------------------------
impl Agent for IpoAgent {
    fn decide_actions(&mut self, view: &MarketView) -> Vec<OrderRequest> {
        if self.has_acted {
            return vec![];
        }
        self.has_acted = true;

        /* choose the first listed instrument */
        let stock_id = match view.stocks.get_all_ids().first() {
            Some(id) => *id,
            None => return vec![], // no universe?
        };

        let num_levels = 20;
        let vol_per = (self.inventory / num_levels) as u64;
        let start_px: u64 = 15_000; // $150.00
        let tick: u64 = 5; // $0.05

        (0..num_levels)
            .map(|i| OrderRequest::LimitOrder {
                agent_id: self.id,
                stock_id,
                side: Side::Sell,
                price: start_px + (i as u64) * tick,
                volume: vol_per,
            })
            .collect()
    }

    /* IPO agent never submits market buys/sells after the ladder ----------- */
    fn buy_stock(&mut self, _id: u64, _v: u64) -> Vec<OrderRequest> {
        vec![]
    }
    fn sell_stock(&mut self, _id: u64, _v: u64) -> Vec<OrderRequest> {
        vec![]
    }

    fn margin_call(&mut self) -> Vec<OrderRequest> {
        vec![]
    }

    /* bookkeeping ---------------------------------------------------------- */
    fn acknowledge_order(&mut self, o: Order) {
        self.open_orders.insert(o.id, o);
    }

    fn update_portfolio(&mut self, vol: i64, tr: &Trade) {
        self.inventory += vol;
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

    /* misc getters --------------------------------------------------------- */
    fn get_id(&self) -> usize {
        self.id
    }
    fn get_inventory(&self) -> i64 {
        self.inventory
    }
    fn clone_agent(&self) -> Box<dyn Agent> {
        Box::new(IpoAgent::new(self.id))
    }

    fn evaluate_port(&mut self, view: &MarketView) -> f64 {
        let sid = *view.stocks.get_all_ids().first().unwrap_or(&0);
        match view.get_mid_price(sid) {
            Some(px) => self.inventory as f64 * (px as f64 / 100.0),
            None => 0.0,
        }
    }
}
