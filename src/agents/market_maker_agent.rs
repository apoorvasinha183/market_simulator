// src/agents/market_maker_agent.rs

use super::agent_trait::{Agent, MarketView};
use super::config::{
    MARGIN_CALL_THRESHOLD,
    // Import all the constants we need
    MM_DESIRED_SPREAD,
    MM_INITIAL_CENTER_PRICE,
    MM_INITIAL_INVENTORY,
    MM_QUOTE_VOL_MAX,
    MM_QUOTE_VOL_MIN,
    MM_SEED_DECAY,
    MM_SEED_DEPTH_PCT,
    MM_SEED_LEVELS,
    MM_SEED_TICK_SPACING,
    MM_SKEW_FACTOR,
    MM_UNSTICK_VOL_MAX,
    MM_UNSTICK_VOL_MIN,
};
//use super::latency;
use crate::agents::latency::MM_TICKS_UNTIL_ACTIVE;
use crate::simulators::order_book::Trade;
use crate::types::order::{Order, OrderRequest, Side};
use rand::Rng;
use std::collections::HashMap;

/// Hard guard‑rails so quotes can never leave a sensible band
const MIN_PRICE: u64 = 1_00; // $1.00  (in cents)
const MAX_PRICE: u64 = 3000_00; // $300.00 (in cents)

#[inline]
fn clamp_price(p: i128) -> u64 {
    p.max(MIN_PRICE as i128).min(MAX_PRICE as i128) as u64
}

pub struct MarketMakerAgent {
    pub id: usize,
    inventory: i64,
    ticks_until_active: u32,
    bootstrapped: bool,
    open_orders: HashMap<u64, Order>,
    #[allow(dead_code)]
    margin: i128,
}

impl MarketMakerAgent {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            inventory: MM_INITIAL_INVENTORY,
            ticks_until_active: MM_TICKS_UNTIL_ACTIVE,
            bootstrapped: false,
            open_orders: HashMap::new(),
            margin: 100000000000,
        }
    }

    /// Seed the book with depth levels using parameters from the config.
    fn seed_liquidity(&self) -> Vec<OrderRequest> {
        let side_budget = (self.inventory.abs() as f64 * MM_SEED_DEPTH_PCT) as u64;
        // Geometric series sum formula to calculate the initial volume
        let mut vol_at_lvl = (side_budget as f64 * (1.0 - MM_SEED_DECAY)
            / (1.0 - MM_SEED_DECAY.powi(MM_SEED_LEVELS as i32)))
            as u64;

        let mut orders = Vec::with_capacity(MM_SEED_LEVELS * 2);
        for lvl in 0..MM_SEED_LEVELS {
            let vol = vol_at_lvl;
            vol_at_lvl = (vol_at_lvl as f64 * MM_SEED_DECAY) as u64;

            let bid_px = clamp_price(
                MM_INITIAL_CENTER_PRICE as i128
                    - (MM_DESIRED_SPREAD / 2 + lvl as u64 * MM_SEED_TICK_SPACING) as i128,
            );
            let ask_px = clamp_price(
                MM_INITIAL_CENTER_PRICE as i128
                    + (MM_DESIRED_SPREAD / 2 + lvl as u64 * MM_SEED_TICK_SPACING) as i128,
            );

            orders.push(OrderRequest::LimitOrder {
                agent_id: self.id,
                side: Side::Buy,
                price: bid_px,
                volume: vol,
            });
            orders.push(OrderRequest::LimitOrder {
                agent_id: self.id,
                side: Side::Sell,
                price: ask_px,
                volume: vol,
            });
        }
        orders
    }
}

impl Agent for MarketMakerAgent {
    fn decide_actions(&mut self, market_view: &MarketView) -> Vec<OrderRequest> {
        if self.ticks_until_active > 0 {
            self.ticks_until_active -= 1;
            return vec![];
        }

        if !self.bootstrapped {
            self.bootstrapped = true;
            return self.seed_liquidity();
        }
        let liquidity = self.evaluate_port(market_view);
        println!("MM has a net position of {}", liquidity);

        let best_bid = market_view.order_book.bids.keys().last().cloned();
        let best_ask = market_view.order_book.asks.keys().next().cloned();

        // --- Emergency «unstick» logic ---
        if let (Some(bid), None) = (best_bid, best_ask) {
            let ask_px = clamp_price((bid as i128) + 1);
            let volume = rand::thread_rng().gen_range(MM_UNSTICK_VOL_MIN..=MM_UNSTICK_VOL_MAX);
            return vec![OrderRequest::LimitOrder {
                agent_id: self.id,
                side: Side::Sell,
                price: ask_px,
                volume,
            }];
        }
        if let (None, Some(ask)) = (best_bid, best_ask) {
            let bid_px = clamp_price((ask as i128) - 1);
            let volume = rand::thread_rng().gen_range(MM_UNSTICK_VOL_MIN..=MM_UNSTICK_VOL_MAX);
            return vec![OrderRequest::LimitOrder {
                agent_id: self.id,
                side: Side::Buy,
                price: bid_px,
                volume,
            }];
        }

        // --- Normal two-sided quoting strategy ---
        let center_price = match (best_bid, best_ask) {
            (Some(bid), Some(ask)) if ask > bid => ((bid as u128 + ask as u128) / 2) as u64,
            (None, None) => MM_INITIAL_CENTER_PRICE,
            _ => return vec![],
        };

        let inventory_skew = (self.inventory as f64 * MM_SKEW_FACTOR) as i64;

        // --- THE FIX: Cast inventory_skew to i128 before subtraction ---
        let our_center_price = clamp_price(center_price as i128 - inventory_skew as i128);

        let our_bid = clamp_price(our_center_price as i128 - (MM_DESIRED_SPREAD / 2) as i128);
        let our_ask = clamp_price(our_center_price as i128 + (MM_DESIRED_SPREAD / 2) as i128);

        if our_ask > our_bid {
            if best_ask.map_or(true, |ask| our_bid < ask)
                && best_bid.map_or(true, |bid| our_ask > bid)
            {
                let volume = rand::thread_rng().gen_range(MM_QUOTE_VOL_MIN..=MM_QUOTE_VOL_MAX);
                return vec![
                    OrderRequest::LimitOrder {
                        agent_id: self.id,
                        side: Side::Buy,
                        price: our_bid,
                        volume,
                    },
                    OrderRequest::LimitOrder {
                        agent_id: self.id,
                        side: Side::Sell,
                        price: our_ask,
                        volume,
                    },
                ];
            }
        }
        vec![]
    }

    fn buy_stock(&mut self, volume: u64) -> Vec<OrderRequest> {
        if let Some(price) = self
            .open_orders
            .values()
            .find(|o| o.side == Side::Sell)
            .map(|o| o.price)
        {
            return vec![OrderRequest::LimitOrder {
                agent_id: self.id,
                side: Side::Buy,
                price,
                volume,
            }];
        }
        vec![]
    }

    fn sell_stock(&mut self, volume: u64) -> Vec<OrderRequest> {
        if let Some(price) = self
            .open_orders
            .values()
            .find(|o| o.side == Side::Buy)
            .map(|o| o.price)
        {
            return vec![OrderRequest::LimitOrder {
                agent_id: self.id,
                side: Side::Sell,
                price,
                volume,
            }];
        }
        vec![]
    }

    fn margin_call(&mut self) -> Vec<OrderRequest> {
        if self.inventory <= MARGIN_CALL_THRESHOLD {
            return vec![OrderRequest::MarketOrder {
                agent_id: self.id,
                side: Side::Buy,
                volume: self.inventory.abs() as u64,
            }];
        }
        vec![]
    }

    fn acknowledge_order(&mut self, order: Order) {
        self.open_orders.insert(order.id, order);
    }

    fn update_portfolio(&mut self, trade_volume: i64, trade: &Trade) {
        self.inventory = self.inventory.saturating_add(trade_volume);
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
        Box::new(MarketMakerAgent::new(self.id))
    }
    fn evaluate_port(&self, market_view: &MarketView) -> f64 {
        let price_cents = match market_view.get_mid_price() {
            Some(p) => p,
            None => return 0.0, // or whatever you deem appropriate
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
    use super::*; // Import from parent module
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
    fn test_update_portfolio_as_maker_partial_fill() {
        // Arrange
        let mut mm = MarketMakerAgent::new(1);
        let order = new_order(101, 1, Side::Sell, 15000, 100);
        mm.acknowledge_order(order); // Agent now tracks this open order.

        let trade = new_trade(2, 1, 101, Side::Buy, 15000, 40);
        let expected_inventory_change = -40; // Maker sold 40 shares.

        // Act
        mm.update_portfolio(expected_inventory_change, &trade);

        // Assert
        let open_order = mm
            .open_orders
            .get(&101)
            .expect("Order 101 should still be open.");
        assert_eq!(
            open_order.filled, 40,
            "The order's filled amount should be 40."
        );
        assert_eq!(
            mm.inventory,
            MM_INITIAL_INVENTORY - 40,
            "Inventory should be reduced by 40."
        );
    }

    #[test]
    fn test_update_portfolio_as_maker_full_fill() {
        // Arrange
        let mut mm = MarketMakerAgent::new(1);
        let order = new_order(101, 1, Side::Sell, 15000, 100);
        mm.acknowledge_order(order);

        let trade = new_trade(2, 1, 101, Side::Buy, 15000, 100);
        let expected_inventory_change = -100;

        // Act
        mm.update_portfolio(expected_inventory_change, &trade);

        // Assert
        assert!(
            mm.open_orders.get(&101).is_none(),
            "Order 101 should be removed after a full fill."
        );
        assert_eq!(
            mm.inventory,
            MM_INITIAL_INVENTORY - 100,
            "Inventory should be reduced by 100."
        );
        assert!(
            mm.get_pending_orders().is_empty(),
            "There should be no pending orders."
        );
    }

    #[test]
    fn test_update_portfolio_as_taker() {
        // Arrange
        let mut mm = MarketMakerAgent::new(1);
        // The MM has no open orders initially. It acts as the aggressor.

        let trade = new_trade(1, 2, 202, Side::Buy, 15000, 75); // MM (agent 1) is the taker.
        let expected_inventory_change = 75; // Taker bought 75 shares.

        // Act
        mm.update_portfolio(expected_inventory_change, &trade);

        // Assert
        assert_eq!(
            mm.inventory,
            MM_INITIAL_INVENTORY + 75,
            "Inventory should increase by 75."
        );
        assert!(
            mm.open_orders.is_empty(),
            "Open orders should be unchanged as the MM was the taker."
        );
    }
}
