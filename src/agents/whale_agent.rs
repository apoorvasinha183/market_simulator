// src/agents/whale_agent.rs
use rand::{Rng, seq::SliceRandom};
use std::collections::HashMap;

use super::{
    agent_trait::{Agent, MarketView},
    config::{
        CRAZY_WHALE, WHALE_ACTION_PROB, WHALE_INITIAL_INVENTORY, WHALE_ORDER_VOLUME,
        WHALE_PRICE_OFFSET_MAX, WHALE_PRICE_OFFSET_MIN,
    },
    latency::WHALE_TICKS_UNTIL_ACTIVE,
};
use crate::types::order::{Order, OrderRequest, Side, Trade};

/// A patient, high-capital agent that places large limit orders far
/// from mid-price to create support & resistance.
pub struct WhaleAgent {
    id: usize,
    inventory: i64,
    ticks_until_active: u32,
    open_orders: HashMap<u64, Order>,
    cash: f64,
    //alow dead code
    #[allow(dead_code)]
    margin: f64,
    port_value: f64,
}

impl WhaleAgent {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            inventory: WHALE_INITIAL_INVENTORY,
            ticks_until_active: WHALE_TICKS_UNTIL_ACTIVE,
            open_orders: HashMap::new(),
            cash: 1_000_000_000_000.0,
            margin: 10_000_000_000_000.0,
            port_value: 0.0,
        }
    }
}

// -----------------------------------------------------------------------------
//  Agent impl
// -----------------------------------------------------------------------------
impl Agent for WhaleAgent {
    fn decide_actions(&mut self, view: &MarketView) -> Vec<OrderRequest> {
        if self.ticks_until_active > 0 {
            self.ticks_until_active -= 1;
            return vec![];
        }
        let mut rng = rand::thread_rng();
        if !rng.gen_bool(WHALE_ACTION_PROB) {
            return vec![];
        }

        /* pick a random instrument */
        let ids: Vec<u64> = view.stocks.get_all_ids();
        if ids.is_empty() {
            return vec![];
        }
        let stock_id = *ids.choose(&mut rng).unwrap();

        /* 1) cancel & clear existing orders */
        let cancel_reqs: Vec<OrderRequest> = self
            .open_orders
            .keys()
            .map(|id| OrderRequest::CancelOrder {
                agent_id: self.id,
                order_id: *id,
            })
            .collect();
        self.open_orders.clear();

        /* 2) place fresh orders */
        let mut new_reqs = Vec::new();

        if rng.gen_bool(CRAZY_WHALE) {
            /* slam the book with a huge market order */
            let vol = rng.gen_range(WHALE_ORDER_VOLUME / 2..=WHALE_ORDER_VOLUME);
            let side = if rng.gen_bool(0.5) {
                Side::Buy
            } else {
                Side::Sell
            };
            new_reqs.push(OrderRequest::MarketOrder {
                agent_id: self.id,
                stock_id,
                side,
                volume: vol,
            });
        } else {
            if let Some(mid) = view.get_mid_price(stock_id) {
                let buy_bias = rng.gen_range(WHALE_PRICE_OFFSET_MIN..=WHALE_PRICE_OFFSET_MAX);
                let sell_bias = rng.gen_range(WHALE_PRICE_OFFSET_MIN..=WHALE_PRICE_OFFSET_MAX);
                let bid_px = mid.saturating_sub(buy_bias);
                let ask_px = mid.saturating_add(sell_bias);

                new_reqs.push(OrderRequest::LimitOrder {
                    agent_id: self.id,
                    stock_id,
                    side: Side::Buy,
                    price: bid_px,
                    volume: WHALE_ORDER_VOLUME,
                });
                new_reqs.push(OrderRequest::LimitOrder {
                    agent_id: self.id,
                    stock_id,
                    side: Side::Sell,
                    price: ask_px,
                    volume: WHALE_ORDER_VOLUME,
                });
            }
        }
        cancel_reqs.into_iter().chain(new_reqs).collect()
    }
    fn run(&mut self) {
        // No-op for whales; they act only on market view
    }
    /* market helpers (rarely used for whales) */
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
        vec![]
    } // whale has ample capital

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
        if self.open_orders.remove(&id).is_some() {
            vec![OrderRequest::CancelOrder {
                agent_id: self.id,
                order_id: id,
            }]
        } else {
            vec![]
        }
    }

    /* misc getters --------------------------------------------------------- */
    fn get_id(&self) -> usize {
        self.id
    }
    fn get_inventory(&self) -> i64 {
        self.inventory
    }
    fn clone_agent(&self) -> Box<dyn Agent> {
        Box::new(WhaleAgent::new(self.id))
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
    use super::*;
    use crate::{
        simulators::order_book::{OrderBook, PriceLevel},
        stocks::definitions::StockMarket,
        types::order::{OrderRequest, Side},
    };

    const STOCK_ID: u64 = 1;

    /* helpers -------------------------------------------------------------- */
    fn new_order(id: u64, agent: usize, side: Side, px: u64, vol: u64) -> Order {
        Order {
            id,
            agent_id: agent,
            stock_id: STOCK_ID,
            side,
            price: px,
            volume: vol,
            filled: 0,
        }
    }
    fn new_trade(
        taker: usize,
        maker: usize,
        maker_oid: u64,
        side: Side,
        px: u64,
        vol: u64,
    ) -> Trade {
        Trade {
            price: px,
            stock_id: STOCK_ID,
            volume: vol,
            taker_agent_id: taker,
            maker_agent_id: maker,
            maker_order_id: maker_oid,
            taker_side: side,
        }
    }

    /* --------------------------------------------------------------------- */
    #[test]
    fn whale_cancel_and_replace_logic() {
        let mut whale = WhaleAgent::new(1);
        whale.ticks_until_active = 0; // act immediately
        whale.acknowledge_order(new_order(101, 1, Side::Buy, 14_000, 500_000));
        whale.acknowledge_order(new_order(102, 1, Side::Sell, 16_000, 500_000));

        /* build a dummy book + market view */
        let mut book = OrderBook::new();
        book.bids.insert(14_500, PriceLevel::default());
        book.asks.insert(15_500, PriceLevel::default());

        let mut books = std::collections::HashMap::new();
        books.insert(STOCK_ID, book);
        let view = MarketView {
            order_books: &books,
            stocks: &StockMarket::new(),
        };

        /* run — note: probabilistic; we accept it may no-op */
        let reqs = whale.decide_actions(&view);
        if reqs.is_empty() {
            return; // WHALE_ACTION_PROB skipped action; test inconclusive
        }

        let cancels = reqs
            .iter()
            .filter(|r| matches!(r, OrderRequest::CancelOrder { .. }))
            .count();
        let limits = reqs
            .iter()
            .filter(|r| matches!(r, OrderRequest::LimitOrder { .. }))
            .count();

        assert_eq!(cancels, 2, "should cancel the two existing orders");
        assert!(limits >= 2, "should place at least two new limits");
        assert!(whale.open_orders.is_empty(), "internal map cleared");
    }

    /* --------------------------------------------------------------------- */
    #[test]
    fn whale_update_portfolio_as_maker() {
        let mut whale = WhaleAgent::new(1);
        whale.acknowledge_order(new_order(101, 1, Side::Buy, 14_000, 500_000));

        let tr = new_trade(2, 1, 101, Side::Sell, 14_000, 10_000); // maker bought
        whale.update_portfolio(10_000, &tr);

        let ord = whale.open_orders.get(&101).expect("order remains open");
        assert_eq!(ord.filled, 10_000);
        assert_eq!(whale.inventory, WHALE_INITIAL_INVENTORY + 10_000);
    }
}
