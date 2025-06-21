// src/agents/dumb_agent.rs
use rand::{Rng, seq::SliceRandom};
use std::collections::HashMap;

use super::{
    agent_trait::{Agent, MarketView},
    config::{
        DUMB_AGENT_ACTION_PROB, DUMB_AGENT_LARGE_VOL_CHANCE, DUMB_AGENT_LARGE_VOL_MAX,
        DUMB_AGENT_LARGE_VOL_MIN, DUMB_AGENT_NUM_TRADERS, DUMB_AGENT_TYPICAL_VOL_MAX,
        DUMB_AGENT_TYPICAL_VOL_MIN,
    },
};
use crate::{
    agents::latency::DUMB_AGENT_TICKS_UNTIL_ACTIVE,
    types::order::{Order, OrderRequest, Side, Trade},
};
//allow cloning
#[derive(Debug, Clone)]
pub struct DumbAgent {
    id: usize,
    // update inventory as a hashmap linking the stock id to the number of shares held .(Signed so I can short)
    inventory: HashMap<u64, i64>,
    ticks_until_active: u32,
    open_orders: HashMap<u64, Order>,
    cash: f64,
    margin: f64,
    port_value: f64,
}

impl DumbAgent {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            // empty inventory hashmap
            inventory: HashMap::new(),
            ticks_until_active: DUMB_AGENT_TICKS_UNTIL_ACTIVE,
            open_orders: HashMap::new(),
            cash: 1_000_000_000.0,
            margin: 4_000_000_000.0,
            port_value: 0.0,
        }
    }
}

// -----------------------------------------------------------------------------
//  Agent impl
// -----------------------------------------------------------------------------
impl Agent for DumbAgent {
    fn decide_actions(&mut self, view: &MarketView) -> Vec<OrderRequest> {
        if self.ticks_until_active > 0 {
            self.ticks_until_active -= 1;
            return vec![];
        }

        let mut rng = rand::thread_rng();
        let mut out = Vec::new();

        /* --- choose a random instrument for this tick --- */
        let universe: Vec<u64> = view.stocks.get_all_ids();
        if universe.is_empty() {
            return out;
        }
        let stock_id = *universe.choose(&mut rng).unwrap();

        for _ in 0..DUMB_AGENT_NUM_TRADERS {
            if rng.gen_bool(DUMB_AGENT_ACTION_PROB) {
                let side = if rng.gen_bool(0.5) {
                    Side::Buy
                } else {
                    Side::Sell
                };

                let volume = if rng.gen_bool(DUMB_AGENT_LARGE_VOL_CHANCE) {
                    rng.gen_range(DUMB_AGENT_LARGE_VOL_MIN..=DUMB_AGENT_LARGE_VOL_MAX)
                } else {
                    rng.gen_range(DUMB_AGENT_TYPICAL_VOL_MIN..=DUMB_AGENT_TYPICAL_VOL_MAX)
                };

                /* --- buying-power check --- */
                if side == Side::Buy {
                    if let Some(px) = view.get_mid_price(stock_id) {
                        let cost = volume as f64 * (px as f64 / 100.0);
                        if cost > self.cash + self.margin {
                            continue; // skip action
                        }
                    }
                }

                let reqs = if side == Side::Buy {
                    self.buy_stock(stock_id, volume)
                } else {
                    self.sell_stock(stock_id, volume)
                };
                out.extend(reqs);
            }
        }
        out
    }
    fn run(&mut self) { // loop decide actions here 
    }
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

    fn margin_call(&mut self) -> Vec<OrderRequest> {
        if self.cash < -self.margin {
            // CREATE AN empty vector to hold the liquidation orders
            let mut liquidation_orders = Vec::new();
            // sweep the inventory hashmap and burn all shares into the lqiuidation orders
            for (&stock_id, &vol) in &self.inventory {
                if vol != 0 {
                    liquidation_orders.push(OrderRequest::MarketOrder {
                        agent_id: self.id,
                        stock_id,
                        side: Side::Sell,
                        volume: vol.unsigned_abs() as u64, // convert to unsigned for market order
                    });
                }
            }
            // clear the inventory
            self.inventory.clear();
            // return the liquidation orders
            return liquidation_orders;
        }

        vec![]
    }

    /* ---------- bookkeeping ---------- */

    fn acknowledge_order(&mut self, order: Order) {
        self.open_orders.insert(order.id, order);
    }

    fn update_portfolio(&mut self, vol: i64, tr: &Trade) {
        // Update the inventory for the specific stock_id
        let stock_id = tr.stock_id;
        // update the hashmap inventory
        let current_inventory = self.inventory.entry(stock_id).or_insert(0);
        *current_inventory += vol;
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

    fn cancel_open_order(&mut self, _id: u64) -> Vec<OrderRequest> {
        vec![] // not implemented
    }

    /* ---------- misc ---------- */

    fn get_id(&self) -> usize {
        self.id
    }
    fn get_inventory(&self) -> i64 {
        // count the total inventory across all stocks
        self.inventory.values().sum()
        //self.inventory
    }
    fn clone_agent(&self) -> Box<dyn Agent> {
        Box::new(self.clone()) // clone the agent while preserving the inventory and stuff.
    }

    fn evaluate_port(&mut self, view: &MarketView) -> f64 {
        // iterate over all stocks in the inventory and calculate the total value
        // take out all the stock and use their mid price
        self.port_value = self.inventory.iter().fold(0.0, |acc, (stock_id, &vol)| {
            if let Some(px) = view.get_mid_price(*stock_id) {
                acc + vol as f64 * (px as f64 / 100.0)
            } else {
                acc
            }
        });
        self.port_value
    }
}

// -----------------------------------------------------------------------------
//  Unit Tests
// -----------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    //use crate::types::order::{OrderRequest, Side};
    use crate::types::order::Side;
    const STOCK_ID: u64 = 1; // arbitrary id for all mock trades

    /* helper: create a Trade */
    fn mock_trade(price: u64, vol: u64) -> Trade {
        Trade {
            price,
            stock_id: STOCK_ID,
            volume: vol,
            taker_agent_id: 1,
            maker_agent_id: 2,
            maker_order_id: 101,
            taker_side: Side::Buy,
        }
    }

    #[test]
    fn cash_updates_on_buy() {
        let mut a = DumbAgent::new(0);
        let cash0 = a.cash;
        let tr = mock_trade(15_000, 10); // $150 × 10
        a.update_portfolio(10, &tr); // buy
        let cost = 10.0 * 150.0;
        assert!((a.cash - (cash0 - cost)).abs() < 1e-9);
        // inventory should increase by 10 shares
        assert_eq!(a.inventory.get(&STOCK_ID).unwrap_or(&0), &10);
        //assert_eq!(a.inventory, 300_000_000 + 10);
    }

    #[test]
    fn cash_updates_on_sell() {
        let mut a = DumbAgent::new(0);
        let cash0 = a.cash;
        let tr = mock_trade(15_000, 10);
        a.update_portfolio(-10, &tr); // sell
        let proceeds = 10.0 * 150.0;
        assert!((a.cash - (cash0 + proceeds)).abs() < 1e-9);
        // inventory should decrease by 10 shares
        assert_eq!(a.inventory.get(&STOCK_ID).unwrap_or(&0), &-10);
        //assert_eq!(a.inventory, 300_000_000 - 10);
    }

    #[test]
    fn margin_call_triggers() {
        let mut a = DumbAgent::new(0);
        a.cash = -4_000_000_000.1; // breach
        a.inventory.insert(0, 500);
        a.inventory.insert(1, 100);

        let reqs = a.margin_call();
        assert_eq!(reqs.len(), 2, "should liquidate all inventory");

        // Collect the liquidation orders into a more testable format
        let mut liquidations = HashMap::new();
        for req in &reqs {
            match req {
                OrderRequest::MarketOrder {
                    agent_id,
                    stock_id,
                    side,
                    volume,
                } => {
                    assert_eq!(*agent_id, a.id);
                    assert_eq!(*side, Side::Sell);
                    liquidations.insert(*stock_id, *volume);
                }
                _ => panic!("Expected MarketOrder"),
            }
        }

        // Verify we got the right liquidations
        assert_eq!(liquidations.get(&0), Some(&500));
        assert_eq!(liquidations.get(&1), Some(&100));
        assert!(a.inventory.is_empty(), "inventory should be cleared");
    }

    #[test]
    fn margin_call_not_triggered_when_safe() {
        let mut good = DumbAgent::new(0);
        good.cash = 1_000.0;

        let mut within = DumbAgent::new(1);
        within.cash = -3_999_999_999.9;

        let mut at_limit = DumbAgent::new(2);
        at_limit.cash = -4_000_000_000.0;

        assert!(good.margin_call().is_empty());
        assert!(within.margin_call().is_empty());
        assert!(at_limit.margin_call().is_empty());
    }
}
