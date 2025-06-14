// src/agents/dumb_agent.rs

use super::agent_trait::{Agent, MarketView};
use super::config::{
    DUMB_AGENT_ACTION_PROB, DUMB_AGENT_LARGE_VOL_CHANCE, DUMB_AGENT_LARGE_VOL_MAX,
    DUMB_AGENT_LARGE_VOL_MIN, DUMB_AGENT_NUM_TRADERS, DUMB_AGENT_TYPICAL_VOL_MAX,
    DUMB_AGENT_TYPICAL_VOL_MIN,
};
use crate::agents::latency::DUMB_AGENT_TICKS_UNTIL_ACTIVE;
use crate::simulators::order_book::Trade;
use crate::types::order::{Order, OrderRequest, Side};
use rand::Rng;
use std::collections::HashMap;

pub struct DumbAgent {
    pub id: usize,
    inventory: i64,
    ticks_until_active: u32,
    open_orders: HashMap<u64, Order>,
    cash: f64,
    margin: f64,
    // --- Restored as requested ---
    port_value: f64,
}

impl DumbAgent {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            inventory: 300_000_000,
            ticks_until_active: DUMB_AGENT_TICKS_UNTIL_ACTIVE,
            open_orders: HashMap::new(),
            cash: 1_000_000_000.0,
            margin: 4_000_000_000.0,
            // --- Restored as requested ---
            port_value: 0.0,
        }
    }
}

impl Agent for DumbAgent {
    fn decide_actions(&mut self, market_view: &MarketView) -> Vec<OrderRequest> {
        if self.ticks_until_active > 0 {
            self.ticks_until_active -= 1;
            return vec![];
        }

        let mut rng = rand::thread_rng();
        let mut requests_this_tick = Vec::new();

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

                // --- Buying Power Check ---
                if side == Side::Buy {
                    if let Some(price_cents) = market_view.get_mid_price() {
                        let estimated_cost = (volume as f64) * (price_cents as f64 / 100.0);
                        let buying_power = self.cash + self.margin;
                        if estimated_cost > buying_power {
                            continue; // Not enough buying power, skip action.
                        }
                    }
                }

                let request = if side == Side::Buy {
                    self.buy_stock(volume)
                } else {
                    self.sell_stock(volume)
                };
                requests_this_tick.extend(request);
            }
        }
        // You can uncomment this to use your evaluation function
        // let _liquidity = self.evaluate_port(market_view);
        // println!("Retail has a net position of {}", _liquidity);
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
        // --- CORRECTED MARGIN CALL LOGIC ---
        // A margin call happens if the cash balance is more negative than the margin limit.
        //println!("NASDQ says MARRRRGIIN CALLL to agent {}! Cash: {:.2}", self.id, self.cash);
        if self.cash < -self.margin {
            if self.inventory > 0 {
                //println!("NASDQ says MARRRRGIIN CALLL to agent {}! Cash: {:.2}", self.id, self.cash);
                return self.sell_stock(self.inventory.unsigned_abs());
            }
        }
        vec![]
    }

    fn acknowledge_order(&mut self, order: Order) {
        self.open_orders.insert(order.id, order);
    }

    fn update_portfolio(&mut self, trade_volume: i64, trade: &Trade) {
        // 1. Update inventory.
        self.inventory += trade_volume;

        // 2. Calculate cash change. A positive trade_volume (buy) decreases cash.
        let cash_change = (trade_volume as f64) * (trade.price as f64 / 100.0);
        self.cash -= cash_change;

        // 3. Update open orders if the agent was the maker.
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

    fn evaluate_port(&mut self, market_view: &MarketView) -> f64 {
        let price_cents = match market_view.get_mid_price() {
            Some(p) => p,
            None => return 0.0,
        };
        let value_cents = (self.inventory as i128)
            .checked_mul(price_cents as i128)
            .expect("portfolio value overflow");
        self.port_value = (value_cents as f64) / 100.0;
        self.port_value
    }
}
// -----------------------------------------------------------------------------
//  Unit Tests
// -----------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::order::Side;

    // Helper to create a mock trade for testing.
    fn new_mock_trade(price_cents: u64, volume: u64) -> Trade {
        Trade {
            price: price_cents,
            volume,
            taker_agent_id: 1,     // Doesn't matter for these tests
            maker_agent_id: 2,     // Doesn't matter for these tests
            maker_order_id: 101,   // Doesn't matter
            taker_side: Side::Buy, // Doesn't matter, we use trade_volume sign
        }
    }

    #[test]
    fn test_update_portfolio_cash_on_buy() {
        // Arrange
        let mut agent = DumbAgent::new(0);
        let initial_cash = agent.cash;
        let trade_price_cents = 15000; // $150.00
        let trade_volume_shares = 10;
        let trade = new_mock_trade(trade_price_cents, trade_volume_shares);
        let cost = (trade_volume_shares as f64) * (trade_price_cents as f64 / 100.0);

        // Act: agent buys, so trade_volume is positive
        agent.update_portfolio(trade_volume_shares as i64, &trade);

        // Assert
        assert_eq!(agent.inventory, 300_000_000 + trade_volume_shares as i64);
        // Using float comparison with a small epsilon
        assert!(
            (agent.cash - (initial_cash - cost)).abs() < 1e-9,
            "Cash should decrease by the cost of the trade."
        );
    }

    #[test]
    fn test_update_portfolio_cash_on_sell() {
        // Arrange
        let mut agent = DumbAgent::new(0);
        let initial_cash = agent.cash;
        let trade_price_cents = 15000; // $150.00
        let trade_volume_shares = 10;
        let trade = new_mock_trade(trade_price_cents, trade_volume_shares);
        let proceeds = (trade_volume_shares as f64) * (trade_price_cents as f64 / 100.0);

        // Act: agent sells, so trade_volume is negative
        agent.update_portfolio(-(trade_volume_shares as i64), &trade);

        // Assert
        assert_eq!(agent.inventory, 300_000_000 - trade_volume_shares as i64);
        assert!(
            (agent.cash - (initial_cash + proceeds)).abs() < 1e-9,
            "Cash should increase by the proceeds of the trade."
        );
    }

    #[test]
    fn test_margin_call_triggers_when_breached() {
        // Arrange
        let mut agent = DumbAgent::new(0);
        agent.cash = -4_000_000_000.1; // Breaches the -4B margin
        agent.inventory = 500; // Has inventory to liquidate

        // Act
        let requests = agent.margin_call();

        // Assert
        assert_eq!(requests.len(), 1, "Should generate one liquidation order.");
        match &requests[0] {
            OrderRequest::MarketOrder {
                agent_id,
                side,
                volume,
            } => {
                assert_eq!(*agent_id, agent.id);
                assert_eq!(*side, Side::Sell);
                assert_eq!(*volume, 500, "Should liquidate the entire inventory.");
            }
            _ => panic!("Expected a MarketOrder"),
        }
    }

    #[test]
    fn test_margin_call_does_not_trigger_when_safe() {
        // Arrange: Agent has positive cash
        let mut agent_positive_cash = DumbAgent::new(0);
        agent_positive_cash.cash = 1_000_000.0;

        // Arrange: Agent has negative cash but is within the margin limit
        let mut agent_within_margin = DumbAgent::new(0);
        agent_within_margin.cash = -3_999_999_999.9;

        // Arrange: Agent is exactly at the margin limit
        let mut agent_at_margin = DumbAgent::new(0);
        agent_at_margin.cash = -4_000_000_000.0;

        // Act
        let requests_positive = agent_positive_cash.margin_call();
        let requests_within = agent_within_margin.margin_call();
        let requests_at_limit = agent_at_margin.margin_call();

        // Assert
        assert!(
            requests_positive.is_empty(),
            "Should not trigger margin call with positive cash."
        );
        assert!(
            requests_within.is_empty(),
            "Should not trigger margin call when within margin limit."
        );
        assert!(
            requests_at_limit.is_empty(),
            "Should not trigger margin call when exactly at the margin limit."
        );
    }
}
