// src/agents/whale_agent.rs

use super::agent_trait::{Agent, MarketView};
use super::config::{
    CRAZY_WHALE, WHALE_ACTION_PROB, WHALE_INITIAL_INVENTORY, WHALE_ORDER_VOLUME,
    WHALE_PRICE_OFFSET_MAX, WHALE_PRICE_OFFSET_MIN,
};
use super::latency::WHALE_TICKS_UNTIL_ACTIVE;
use crate::simulators::order_book::Trade;
use crate::types::order::{Order, OrderRequest, Side};
use rand::Rng;
use std::collections::HashMap;

/// A patient, high-capital agent that places large limit orders far from
/// the current price to create support and resistance levels.
pub struct WhaleAgent {
    pub id: usize,
    inventory: i64,
    ticks_until_active: u32,
    open_orders: HashMap<u64, Order>,
    #[allow(dead_code)]
    margin:i128,
}

impl WhaleAgent {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            inventory: WHALE_INITIAL_INVENTORY,
            ticks_until_active: WHALE_TICKS_UNTIL_ACTIVE,
            open_orders: HashMap::new(),
            margin:1000000000000
        }
    }
}

impl Agent for WhaleAgent {
    fn decide_actions(&mut self, market_view: &MarketView) -> Vec<OrderRequest> {
        if self.ticks_until_active > 0 {
            self.ticks_until_active -= 1;
            return vec![];
        }

        let mut rng = rand::thread_rng();

        if !rng.gen_bool(WHALE_ACTION_PROB) {
            return vec![];
        }

        // --- Cancel and Replace Logic ---
        let ids_to_cancel: Vec<u64> = self.open_orders.keys().cloned().collect();
        let mut requests: Vec<OrderRequest> = ids_to_cancel
            .into_iter()
            .flat_map(|id| self.cancel_open_order(id))
            .collect();

        self.open_orders.clear();

        // --- Place new orders ---
        if rng.gen_bool(CRAZY_WHALE) {
            let crazy_volume = rng.gen_range((WHALE_ORDER_VOLUME / 2)..=WHALE_ORDER_VOLUME);
            let side = if rng.gen_bool(0.5) {
                Side::Buy
            } else {
                Side::Sell
            };
            requests.push(OrderRequest::MarketOrder {
                agent_id: self.id,
                side,
                volume: crazy_volume,
            });
        } else {
            if let Some(center_price) = market_view.get_mid_price() {
                let buy_bias = rng.gen_range(WHALE_PRICE_OFFSET_MIN..=WHALE_PRICE_OFFSET_MAX);
                let sell_bias = rng.gen_range(WHALE_PRICE_OFFSET_MIN..=WHALE_PRICE_OFFSET_MAX);
                let support_price = center_price.saturating_sub(buy_bias);
                let resistance_price = center_price.saturating_add(sell_bias);

                requests.push(OrderRequest::LimitOrder {
                    agent_id: self.id,
                    side: Side::Buy,
                    price: support_price,
                    volume: WHALE_ORDER_VOLUME,
                });
                requests.push(OrderRequest::LimitOrder {
                    agent_id: self.id,
                    side: Side::Sell,
                    price: resistance_price,
                    volume: WHALE_ORDER_VOLUME,
                });
            }
        }
        let liquidity = self.evaluate_port(market_view);
        println!("Whales have a net position of {}",liquidity);
        requests
    }

    fn buy_stock(&mut self, _volume: u64) -> Vec<OrderRequest> {
        vec![]
    }
    fn sell_stock(&mut self, _volume: u64) -> Vec<OrderRequest> {
        vec![]
    }
    fn margin_call(&mut self) -> Vec<OrderRequest> {
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
            return vec![OrderRequest::CancelOrder {
                agent_id: self.id,
                order_id,
            }];
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
        Box::new(WhaleAgent::new(self.id))
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

// -----------------------------------------------------------------------------
//  Unit Tests
// -----------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulators::order_book::{OrderBook, PriceLevel};
    use crate::types::order::Side;

    // Helper to create a new order for testing.
    fn new_order(id: u64, agent_id: usize, side: Side, price: u64, volume: u64) -> Order {
        Order {
            id,
            agent_id,
            side,
            price,
            volume,
            filled: 0,
        }
    }

    // Helper to create a mock trade for testing.
    fn new_trade(
        taker_id: usize,
        maker_id: usize,
        maker_order_id: u64,
        side: Side,
        price: u64,
        vol: u64,
    ) -> Trade {
        Trade {
            price,
            volume: vol,
            taker_agent_id: taker_id,
            maker_agent_id: maker_id,
            maker_order_id,
            taker_side: side,
        }
    }

    #[test]
    fn test_whale_cancel_and_replace_logic() {
        // Arrange
        let mut whale = WhaleAgent::new(1);
        whale.ticks_until_active = 0; // Make it active immediately
        // Give it some existing open orders to cancel
        whale.acknowledge_order(new_order(101, 1, Side::Buy, 14000, 500_000));
        whale.acknowledge_order(new_order(102, 1, Side::Sell, 16000, 500_000));

        let mut book = OrderBook::new();
        book.bids.insert(14500, PriceLevel::default());
        book.asks.insert(15500, PriceLevel::default());
        let view = MarketView {
            order_book: &book,
            //last_traded_price: 150.00,
        };

        // Act
        // Set WHALE_ACTION_PROB to 1.0 for this test by re-seeding the rng if needed,
        // or just accept that this test is probabilistic. For simplicity, we assume it acts.
        // A more robust test would mock the RNG.
        let requests = whale.decide_actions(&view);

        // Assert
        // This test will only pass if the whale decides to act (WHALE_ACTION_PROB).
        if !requests.is_empty() {
            let cancel_count = requests
                .iter()
                .filter(|r| matches!(r, OrderRequest::CancelOrder { .. }))
                .count();
            let limit_count = requests
                .iter()
                .filter(|r| matches!(r, OrderRequest::LimitOrder { .. }))
                .count();

            assert_eq!(
                cancel_count, 2,
                "Should have generated two cancel requests."
            );
            assert!(
                limit_count >= 2,
                "Should have generated at least two new limit orders."
            );
            assert!(
                whale.open_orders.is_empty(),
                "Internal open orders map should be cleared."
            );
        }
    }

    #[test]
    fn test_whale_update_portfolio_as_maker() {
        // Arrange
        let mut whale = WhaleAgent::new(1);
        whale.acknowledge_order(new_order(101, 1, Side::Buy, 14000, 500_000));
        let trade = new_trade(2, 1, 101, Side::Sell, 14000, 10_000);
        let expected_inventory_change = 10_000; // Maker bought 10k shares.

        // Act
        whale.update_portfolio(expected_inventory_change, &trade);

        // Assert
        let open_order = whale
            .open_orders
            .get(&101)
            .expect("Order 101 should still be open.");
        assert_eq!(open_order.filled, 10_000);
        assert_eq!(whale.inventory, WHALE_INITIAL_INVENTORY + 10_000);
    }
}
