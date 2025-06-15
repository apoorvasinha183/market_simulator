// src/agents/market_maker_agent.rs

use crate::stocks::definitions::Symbol;
use super::agent_trait::{Agent, MarketView};
use super::config::{
    MM_DESIRED_SPREAD, MM_INITIAL_INVENTORY, MM_QUOTE_VOL_MAX, MM_QUOTE_VOL_MIN,
    MM_SEED_DECAY, MM_SEED_DEPTH_PCT, MM_SEED_LEVELS, MM_SEED_TICK_SPACING, MM_SKEW_FACTOR,
};
use crate::agents::latency::MM_TICKS_UNTIL_ACTIVE;
use crate::simulators::order_book::Trade;
use crate::types::order::{Order, OrderRequest, Side};
use rand::Rng;
use std::collections::HashMap;

/// Hard guard‑rails so quotes can never leave a sensible band
const MIN_PRICE: u64 = 1_00; // $1.00  (in cents)
const MAX_PRICE: u64 = 300_000; // $300.00 (in cents)

#[inline]
fn clamp_price(p: i128) -> u64 {
    p.max(MIN_PRICE as i128).min(MAX_PRICE as i128) as u64
}

pub struct MarketMakerAgent {
    pub id: usize,
    inventory: HashMap<Symbol, i64>,
    ticks_until_active: u32,
    bootstrapped: bool,
    open_orders: HashMap<u64, Order>,
    cash: f64,
    margin: f64,
    port_value: f64,
}

impl MarketMakerAgent {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            inventory: HashMap::new(),
            ticks_until_active: MM_TICKS_UNTIL_ACTIVE,
            bootstrapped: false,
            open_orders: HashMap::new(),
            cash: 100_000_000_000.0,
            margin: 400_000_000_000.0,
            port_value: 0.0,
        }
    }

    // FIXED: Corrected type casting and iteration.
    fn seed_liquidity(&mut self, market_view: &MarketView) -> Vec<OrderRequest> {
        let mut orders = Vec::new();
        let total_inventory_value_target = MM_INITIAL_INVENTORY;
        
        // FIXED: Use .iter() to iterate over the HashMap correctly.
        for (symbol, book) in market_view.order_books.iter() {
            let center_price = book.bids.keys().last().cloned().unwrap_or(15000);
            let side_budget_shares = total_inventory_value_target / market_view.order_books.len() as i64;
            let side_budget_value = side_budget_shares as f64 * (center_price as f64 / 100.0);
            
            let mut vol_at_lvl = (side_budget_value * MM_SEED_DEPTH_PCT * (1.0 - MM_SEED_DECAY)
                / (1.0 - MM_SEED_DECAY.powi(MM_SEED_LEVELS as i32))) as u64;

            for lvl in 0..MM_SEED_LEVELS {
                let vol = vol_at_lvl;
                vol_at_lvl = (vol_at_lvl as f64 * MM_SEED_DECAY) as u64;

                // FIXED: Cast `lvl` (which is usize) to u64 for arithmetic.
                let bid_px = clamp_price(center_price as i128 - (MM_DESIRED_SPREAD / 2 + lvl as u64 * MM_SEED_TICK_SPACING) as i128);
                let ask_px = clamp_price(center_price as i128 + (MM_DESIRED_SPREAD / 2 + lvl as u64 * MM_SEED_TICK_SPACING) as i128);

                orders.push(OrderRequest::LimitOrder {
                    symbol: symbol.clone(), agent_id: self.id, side: Side::Buy, price: bid_px, volume: vol,
                });
                orders.push(OrderRequest::LimitOrder {
                    symbol: symbol.clone(), agent_id: self.id, side: Side::Sell, price: ask_px, volume: vol,
                });
            }
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
            return self.seed_liquidity(market_view);
        }

        let mut all_requests = Vec::new();

        // FIXED: Use .iter() to iterate correctly over the HashMap.
        for (symbol, order_book) in market_view.order_books.iter() {
            let best_bid = order_book.bids.keys().last().cloned();
            let best_ask = order_book.asks.keys().next().cloned();

            if let (Some(bid), Some(ask)) = (best_bid, best_ask) {
                 if ask <= bid { continue; }

                let center_price = (bid + ask) / 2;
                let inventory_for_symbol = self.inventory.get(symbol).cloned().unwrap_or(0);
                let inventory_skew = (inventory_for_symbol as f64 * MM_SKEW_FACTOR) as i128;

                let our_center_price = clamp_price(center_price as i128 - inventory_skew);
                let our_bid = clamp_price(our_center_price as i128 - (MM_DESIRED_SPREAD / 2) as i128);
                let our_ask = clamp_price(our_center_price as i128 + (MM_DESIRED_SPREAD / 2) as i128);

                if our_ask > our_bid {
                    let volume = rand::thread_rng().gen_range(MM_QUOTE_VOL_MIN..=MM_QUOTE_VOL_MAX);
                    all_requests.push(OrderRequest::LimitOrder {
                        symbol: symbol.clone(), agent_id: self.id, side: Side::Buy, price: our_bid, volume,
                    });
                    all_requests.push(OrderRequest::LimitOrder {
                        symbol: symbol.clone(), agent_id: self.id, side: Side::Sell, price: our_ask, volume,
                    });
                }
            }
        }
        all_requests
    }

    fn buy_stock(&mut self, volume: u64, symbol: &Symbol) -> Vec<OrderRequest> {
        if let Some(price) = self.open_orders.values().find(|o| o.side == Side::Sell && o.symbol == *symbol).map(|o| o.price) {
            return vec![OrderRequest::LimitOrder {
                symbol: symbol.clone(), agent_id: self.id, side: Side::Buy, price, volume,
            }];
        }
        vec![]
    }

    fn sell_stock(&mut self, volume: u64, symbol: &Symbol) -> Vec<OrderRequest> {
        if let Some(price) = self.open_orders.values().find(|o| o.side == Side::Buy && o.symbol == *symbol).map(|o| o.price) {
            return vec![OrderRequest::LimitOrder {
                symbol: symbol.clone(), agent_id: self.id, side: Side::Sell, price, volume,
            }];
        }
        vec![]
    }

    // FIXED: Use collect-then-iterate pattern to fix borrow error.
    fn margin_call(&mut self) -> Vec<OrderRequest> {
        if self.cash < -self.margin {
            let to_liquidate: Vec<(Symbol, i64)> = self.inventory.iter()
                .map(|(s, &a)| (s.clone(), a)).collect();
            
            let mut requests = Vec::new();
            for (symbol, amount) in to_liquidate {
                if amount > 0 {
                    requests.extend(self.sell_stock(amount.unsigned_abs(), &symbol));
                } else if amount < 0 {
                    requests.extend(self.buy_stock(amount.unsigned_abs(), &symbol));
                }
            }
            if !requests.is_empty() { println!("Liquidation for MM agent {}!", self.id); }
            return requests;
        }
        vec![]
    }
    
    fn acknowledge_order(&mut self, order: Order) {
        self.open_orders.insert(order.id, order);
    }

    fn update_portfolio(&mut self, trade_volume: i64, trade: &Trade) {
        let inventory_for_symbol = self.inventory.entry(trade.symbol.clone()).or_insert(0);
        *inventory_for_symbol = inventory_for_symbol.saturating_add(trade_volume);

        let cash_change = (trade_volume as f64) * (trade.price as f64 / 100.0);
        self.cash -= cash_change;

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
        if let Some(order) = self.open_orders.get(&order_id) {
             return vec![OrderRequest::CancelOrder {
                symbol: order.symbol.clone(),
                agent_id: self.id,
                order_id,
            }];
        }
        vec![]
    }

    fn get_id(&self) -> usize {
        self.id
    }

    fn get_inventory(&self) -> &HashMap<Symbol, i64> {
        &self.inventory
    }

    fn clone_agent(&self) -> Box<dyn Agent> {
        Box::new(MarketMakerAgent::new(self.id))
    }
    
    fn evaluate_port(&mut self, market_view: &MarketView) -> f64 {
        let mut total_value = 0.0;
        for (symbol, &amount) in &self.inventory {
            if let Some(price_cents) = market_view.get_mid_price(symbol) {
                let value_cents = (amount as i128) * (price_cents as i128);
                total_value += (value_cents as f64) / 100.0;
            }
        }
        self.port_value = total_value;
        self.port_value
    }
}
