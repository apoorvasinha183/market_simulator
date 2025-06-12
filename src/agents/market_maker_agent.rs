// src/agents/market_maker_agent.rs

use super::agent_trait::{Agent, MarketView};
use crate::types::order::{Order, OrderRequest, Side};
use rand::Rng;
use std::collections::HashMap;

/// Hard guard-rails so quotes can never leave a sensible band
const MIN_PRICE: u64 = 1_00;     // $1.00  (in cents)
const MAX_PRICE: u64 = 300_00; // $300.00 (in cents)

#[inline]
fn clamp_price(p: i128) -> u64 {
    p.max(MIN_PRICE as i128).min(MAX_PRICE as i128) as u64
}

pub struct MarketMakerAgent {
    pub id: usize,
    inventory: i64,
    desired_spread: u64,
    skew_factor: f64,
    initial_center_price: u64,
    ticks_until_active: u32,
    bootstrapped: bool,
    /// Agent now statefully tracks its own open orders.
    open_orders: HashMap<u64, Order>,
}

impl MarketMakerAgent {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            inventory: 1_000_000,
            desired_spread: 25,
            skew_factor: 0.00001, // Skew factor is now much smaller relative to large inventory
            initial_center_price: 15_000,
            ticks_until_active: 5,
            bootstrapped: false,
            open_orders: HashMap::new(),
        }
    }

    /// Build an initial depth ladder (10 levels either side, tapering volume).
    fn seed_liquidity(&self) -> Vec<OrderRequest> {
        let mut orders = Vec::with_capacity(20);
        let base_vol: u64 = 30_000;
        let levels: u64 = 10;

        for lvl in 0..levels {
            let vol = base_vol.saturating_sub(lvl * 2_000);
            let bid_px = clamp_price(
                self.initial_center_price as i128 - (self.desired_spread / 2 + lvl * 5) as i128,
            );
            let ask_px = clamp_price(
                self.initial_center_price as i128 + (self.desired_spread / 2 + lvl * 5) as i128,
            );

            orders.push(OrderRequest::LimitOrder { agent_id: self.id, side: Side::Buy, price: bid_px, volume: vol });
            orders.push(OrderRequest::LimitOrder { agent_id: self.id, side: Side::Sell, price: ask_px, volume: vol });
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

        let best_bid = market_view.order_book.bids.keys().last().cloned();
        let best_ask = market_view.order_book.asks.keys().next().cloned();

        let center_price = match (best_bid, best_ask) {
            (Some(bid), Some(ask)) if ask > bid => ((bid as u128 + ask as u128) / 2) as u64,
            (None, Some(ask)) => ask.saturating_sub(self.desired_spread),
            (Some(bid), None) => bid.saturating_add(self.desired_spread),
            _ => return vec![],
        };

        let inventory_skew = (self.inventory as f64 * self.skew_factor) as i64;
        let our_center_price = clamp_price(center_price as i128 - inventory_skew as i128);
        let our_bid = clamp_price(our_center_price as i128 - (self.desired_spread / 2) as i128);
        let our_ask = clamp_price(our_center_price as i128 + (self.desired_spread / 2) as i128);

        if our_ask > our_bid {
            if best_ask.map_or(true, |ask| our_bid < ask) && best_bid.map_or(true, |bid| our_ask > bid) {
                let volume = rand::thread_rng().gen_range(50_000..=100_000);
                // This agent primarily uses limit orders to quote the market
                return vec![
                    OrderRequest::LimitOrder { agent_id: self.id, side: Side::Buy, price: our_bid, volume },
                    OrderRequest::LimitOrder { agent_id: self.id, side: Side::Sell, price: our_ask, volume },
                ];
            }
        }
        vec![]
    }

    // --- Fulfillment of the new Agent trait contract ---

    fn buy_stock(&mut self, volume: u64) -> Vec<OrderRequest> {
        // A Market Maker typically wouldn't place large directional market orders,
        // but it could place an aggressive limit order to acquire inventory.
        // This is a placeholder for more complex logic.
        if let Some(price) = self.open_orders.values().find(|o| o.side == Side::Sell).map(|o| o.price) {
             return vec![OrderRequest::LimitOrder { agent_id: self.id, side: Side::Buy, price, volume }];
        }
        vec![]
    }

    fn sell_stock(&mut self, volume: u64) -> Vec<OrderRequest> {
        if let Some(price) = self.open_orders.values().find(|o| o.side == Side::Buy).map(|o| o.price) {
            return vec![OrderRequest::LimitOrder { agent_id: self.id, side: Side::Sell, price, volume }];
        }
        vec![]
    }

    fn margin_call(&mut self) -> Vec<OrderRequest> {
        // Moved the short-covering logic here from decide_actions.
        if self.inventory <= -20_000 {
            println!("!!! MarketMaker {} MARGIN CALL! Covering short of {} !!!", self.id, self.inventory);
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

    fn update_portfolio(&mut self, trade_volume: i64) {
        self.inventory = self.inventory.saturating_add(trade_volume);
        // Here we would also update the state of our open_orders map
        // by removing or reducing the volume of filled orders.
    }

    fn get_pending_orders(&self) -> Vec<Order> {
        self.open_orders.values().cloned().collect()
    }

    fn cancel_open_order(&mut self, order_id: u64) -> Vec<OrderRequest> {
        if self.open_orders.remove(&order_id).is_some() {
            println!("MarketMaker {} requesting cancel for order {}", self.id, order_id);
            // This would return a real Cancel request in a full implementation
        }
        vec![]
    }

    fn get_id(&self) -> usize { self.id }
    fn get_inventory(&self) -> i64 { self.inventory }
    fn clone_agent(&self) -> Box<dyn Agent> { Box::new(MarketMakerAgent::new(self.id)) }
}
