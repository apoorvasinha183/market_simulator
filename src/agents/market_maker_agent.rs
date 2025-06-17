// src/agents/market_maker_agent.rs
use rand::{Rng, seq::SliceRandom};
use std::collections::HashMap;

use super::{
    agent_trait::{Agent, MarketView},
    config::{
        MM_DESIRED_SPREAD, MM_INITIAL_CENTER_PRICE, MM_INITIAL_INVENTORY, MM_QUOTE_VOL_MAX,
        MM_QUOTE_VOL_MIN, MM_SEED_DECAY, MM_SEED_DEPTH_PCT, MM_SEED_LEVELS, MM_SEED_TICK_SPACING,
        MM_SKEW_FACTOR, MM_UNSTICK_VOL_MAX, MM_UNSTICK_VOL_MIN,
    },
};
use crate::{
    agents::latency::MM_TICKS_UNTIL_ACTIVE,
    types::order::{Order, OrderRequest, Side, Trade},
};

/* guard-rails */
const MIN_PRICE: u64 = 1_00; // $1.00
const MAX_PRICE: u64 = 3_000_00; // $3 000.00
#[inline]
fn clamp(p: i128) -> u64 {
    p.max(MIN_PRICE as i128).min(MAX_PRICE as i128) as u64
}

pub struct MarketMakerAgent {
    id: usize,
    inventory: i64,
    ticks_until_active: u32,
    bootstrapped: HashMap<u64, bool>, // per-stock seeding status
    open_orders: HashMap<u64, Order>,
    cash: f64,
    margin: f64,
    port_value: f64,
}

impl MarketMakerAgent {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            inventory: MM_INITIAL_INVENTORY,
            ticks_until_active: MM_TICKS_UNTIL_ACTIVE,
            bootstrapped: HashMap::new(),
            open_orders: HashMap::new(),
            cash: 100_000_000_000.0,
            margin: 400_000_000_000.0,
            port_value: 0.0,
        }
    }

    /* seed one instrument’s book with geometric depth */
    // Adding one more argument that is the opening stock price.
    fn seed_liquidity(&self, stock_id: u64, starting_price: u64) -> Vec<OrderRequest> {
        let side_budget = (self.inventory.abs() as f64 * MM_SEED_DEPTH_PCT) as u64;
        let mut vol_at_lvl = (side_budget as f64 * (1.0 - MM_SEED_DECAY)
            / (1.0 - MM_SEED_DECAY.powi(MM_SEED_LEVELS as i32)))
            as u64;

        let mut out = Vec::with_capacity(MM_SEED_LEVELS * 2);
        for lvl in 0..MM_SEED_LEVELS {
            let vol = vol_at_lvl;
            vol_at_lvl = (vol_at_lvl as f64 * MM_SEED_DECAY) as u64;

            let bid_px = clamp(
                starting_price as i128
                    - (MM_DESIRED_SPREAD / 2 + lvl as u64 * MM_SEED_TICK_SPACING) as i128,
            );
            let ask_px = clamp(
                starting_price as i128
                    + (MM_DESIRED_SPREAD / 2 + lvl as u64 * MM_SEED_TICK_SPACING) as i128,
            );

            out.push(OrderRequest::LimitOrder {
                agent_id: self.id,
                stock_id,
                side: Side::Buy,
                price: bid_px,
                volume: vol,
            });
            out.push(OrderRequest::LimitOrder {
                agent_id: self.id,
                stock_id,
                side: Side::Sell,
                price: ask_px,
                volume: vol,
            });
        }
        out
    }
}

// -----------------------------------------------------------------------------
//  Agent impl
// -----------------------------------------------------------------------------
impl Agent for MarketMakerAgent {
    fn decide_actions(&mut self, view: &MarketView) -> Vec<OrderRequest> {
        if self.ticks_until_active > 0 {
            self.ticks_until_active -= 1;
            return vec![];
        }

        /* pick a random instrument each tick */
        let ids: Vec<u64> = view.stocks.get_all_ids();
        if ids.is_empty() {
            return vec![];
        }
        let stock_id = *ids.choose(&mut rand::thread_rng()).unwrap();
        // extract stock initial price by first fetching the stock by id and extracting rpcie from there
        let initial_price = view
            .stocks
            .get_stock_by_id(stock_id)
            .map(|s| (s.initial_price * 100.0) as u64)
            .unwrap_or(MM_INITIAL_CENTER_PRICE);
        let book = match view.book(stock_id) {
            Some(b) => b,
            None => return vec![],
        };

        /* one-time seeding per instrument */
        if !*self.bootstrapped.entry(stock_id).or_insert(false) {
            self.bootstrapped.insert(stock_id, true);
            return self.seed_liquidity(stock_id, initial_price);
        }

        let best_bid = book.bids.keys().next_back().copied();
        let best_ask = book.asks.keys().next().copied();

        /* --- emergency unstick --- */
        if let (Some(bid), None) = (best_bid, best_ask) {
            let ask_px = clamp(bid as i128 + 1);
            let vol = rand::thread_rng().gen_range(MM_UNSTICK_VOL_MIN..=MM_UNSTICK_VOL_MAX);
            return vec![OrderRequest::LimitOrder {
                agent_id: self.id,
                stock_id,
                side: Side::Sell,
                price: ask_px,
                volume: vol,
            }];
        }
        if let (None, Some(ask)) = (best_bid, best_ask) {
            let bid_px = clamp(ask as i128 - 1);
            let vol = rand::thread_rng().gen_range(MM_UNSTICK_VOL_MIN..=MM_UNSTICK_VOL_MAX);
            return vec![OrderRequest::LimitOrder {
                agent_id: self.id,
                stock_id,
                side: Side::Buy,
                price: bid_px,
                volume: vol,
            }];
        }

        /* --- regular two-sided quote --- */
        let center = match (best_bid, best_ask) {
            (Some(b), Some(a)) if a > b => ((b as u128 + a as u128) / 2) as u64,
            (None, None) => MM_INITIAL_CENTER_PRICE,
            _ => return vec![],
        };

        let inventory_skew = (self.inventory as f64 * MM_SKEW_FACTOR) as i64;
        let our_center = clamp(center as i128 - inventory_skew as i128);

        let bid_px = clamp(our_center as i128 - (MM_DESIRED_SPREAD / 2) as i128);
        let ask_px = clamp(our_center as i128 + (MM_DESIRED_SPREAD / 2) as i128);

        if ask_px <= bid_px {
            return vec![];
        }
        if best_ask.map_or(false, |a| bid_px >= a) || best_bid.map_or(false, |b| ask_px <= b) {
            return vec![];
        }

        let vol = rand::thread_rng().gen_range(MM_QUOTE_VOL_MIN..=MM_QUOTE_VOL_MAX);
        vec![
            OrderRequest::LimitOrder {
                agent_id: self.id,
                stock_id,
                side: Side::Buy,
                price: bid_px,
                volume: vol,
            },
            OrderRequest::LimitOrder {
                agent_id: self.id,
                stock_id,
                side: Side::Sell,
                price: ask_px,
                volume: vol,
            },
        ]
    }

    /* market-order helpers: use inside liquidation paths */
    fn buy_stock(&mut self, stock_id: u64, vol: u64) -> Vec<OrderRequest> {
        vec![OrderRequest::MarketOrder {
            agent_id: self.id,
            stock_id,
            side: Side::Buy,
            volume: vol,
        }]
    }
    fn sell_stock(&mut self, stock_id: u64, vol: u64) -> Vec<OrderRequest> {
        vec![OrderRequest::MarketOrder {
            agent_id: self.id,
            stock_id,
            side: Side::Sell,
            volume: vol,
        }]
    }

    fn margin_call(&mut self) -> Vec<OrderRequest> {
        if self.cash <= -self.margin {
            /* sell entire inventory on the first id we hold */
            let sid = self
                .open_orders
                .values()
                .next()
                .map(|o| o.stock_id)
                .unwrap_or(0);
            return self.sell_stock(sid, self.inventory.unsigned_abs());
        }
        vec![]
    }

    /* bookkeeping ---------------------------------------------------------- */
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

    /* misc getters --------------------------------------------------------- */
    fn get_id(&self) -> usize {
        self.id
    }
    fn get_inventory(&self) -> i64 {
        self.inventory
    }
    fn clone_agent(&self) -> Box<dyn Agent> {
        Box::new(MarketMakerAgent::new(self.id))
    }

    fn evaluate_port(&mut self, view: &MarketView) -> f64 {
        let sid = *view.stocks.get_all_ids().first().unwrap_or(&0);
        if let Some(px) = view.get_mid_price(sid) {
            self.port_value = self.inventory as f64 * (px as f64 / 100.0);
        }
        self.port_value
    }
}
// -----------------------------------------------------------------------------
//  Unit Tests
// -----------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*; // test the agent in this file
    use crate::types::order::Side;

    const STOCK_ID: u64 = 1; // arbitrary instrument key

    /* helper: make an Order */
    fn new_order(id: u64, agent: usize, side: Side, price: u64, vol: u64) -> Order {
        Order {
            id,
            agent_id: agent,
            stock_id: STOCK_ID,
            side,
            price,
            volume: vol,
            filled: 0,
        }
    }

    /* helper: make a Trade */
    fn new_trade(
        taker: usize,
        maker: usize,
        maker_ord: u64,
        side: Side,
        price: u64,
        vol: u64,
    ) -> Trade {
        Trade {
            price,
            stock_id: STOCK_ID,
            volume: vol,
            taker_agent_id: taker,
            maker_agent_id: maker,
            maker_order_id: maker_ord,
            taker_side: side,
        }
    }

    #[test]
    fn maker_partial_fill_updates_open_order() {
        let mut mm = MarketMakerAgent::new(1);
        mm.acknowledge_order(new_order(101, 1, Side::Sell, 15_000, 100));

        let tr = new_trade(2, 1, 101, Side::Buy, 15_000, 40);
        mm.update_portfolio(-40, &tr); // maker sold 40

        let ord = mm.open_orders.get(&101).expect("order still open");
        assert_eq!(ord.filled, 40);
        assert_eq!(mm.inventory, MM_INITIAL_INVENTORY - 40);
    }

    #[test]
    fn maker_full_fill_removes_order() {
        let mut mm = MarketMakerAgent::new(1);
        mm.acknowledge_order(new_order(101, 1, Side::Sell, 15_000, 100));

        let tr = new_trade(2, 1, 101, Side::Buy, 15_000, 100);
        mm.update_portfolio(-100, &tr);

        assert!(mm.open_orders.get(&101).is_none(), "order closed");
        assert_eq!(mm.inventory, MM_INITIAL_INVENTORY - 100);
        assert!(mm.get_pending_orders().is_empty());
    }

    #[test]
    fn taker_trade_leaves_open_orders_untouched() {
        let mut mm = MarketMakerAgent::new(1);

        let tr = new_trade(1, 2, 202, Side::Buy, 15_000, 75); // mm is taker
        mm.update_portfolio(75, &tr);

        assert_eq!(mm.inventory, MM_INITIAL_INVENTORY + 75);
        assert!(mm.open_orders.is_empty());
    }
}
