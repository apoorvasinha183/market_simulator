// src/agents/whale_agent.rs

use crate::stocks::definitions::Symbol;
use super::agent_trait::Agent;
use super::config::{
    CRAZY_WHALE, WHALE_ACTION_PROB, WHALE_INITIAL_INVENTORY, WHALE_ORDER_VOLUME,
    WHALE_PRICE_OFFSET_MAX, WHALE_PRICE_OFFSET_MIN,
};
// FIXED: This import is no longer needed after correcting the typo.
// use crate::agents::latency::WHALE_AGENT_TICKS_UNTIL_ACTIVE;
use crate::{MarketView, Order, OrderRequest, Side, Trade};
use rand::Rng;
use std::collections::HashMap;

/// A patient, high-capital agent that places large limit orders far from
/// the current price to create support and resistance levels.
pub struct WhaleAgent {
    pub id: usize,
    inventory: HashMap<Symbol, i64>,
    ticks_until_active: u32,
    open_orders: HashMap<u64, Order>,
    cash: f64,
    margin: f64,
    port_value: f64,
}

impl WhaleAgent {
    pub fn new(id: usize) -> Self {
        let mut inventory = HashMap::new();
        inventory.insert("AAPL".to_string(), WHALE_INITIAL_INVENTORY);
        
        Self {
            id,
            inventory,
            // FIXED: Corrected typo from WHALE_TICKS_UNTIL_ACTIVE to WHALE_AGENT_TICKS_UNTIL_ACTIVE.
            ticks_until_active: crate::agents::latency::WHALE_AGENT_TICKS_UNTIL_ACTIVE,
            open_orders: HashMap::new(),
            cash: 1_000_000_000_000.0,
            margin: 10_000_000_000_000.0,
            port_value: 0.0,
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

        let ids_to_cancel: Vec<u64> = self.open_orders.keys().cloned().collect();
        let mut requests: Vec<OrderRequest> = ids_to_cancel
            .into_iter()
            .flat_map(|id| self.cancel_open_order(id))
            .collect();

        self.open_orders.clear();

        let Some(symbol_to_trade) = market_view.order_books.keys().next().cloned() else {
            return vec![];
        };

        if rng.gen_bool(CRAZY_WHALE) {
            let crazy_volume = rng.gen_range((WHALE_ORDER_VOLUME / 2)..=WHALE_ORDER_VOLUME);
            let side = if rng.gen_bool(0.5) { Side::Buy } else { Side::Sell };
            requests.push(OrderRequest::MarketOrder {
                symbol: symbol_to_trade,
                agent_id: self.id,
                side,
                volume: crazy_volume,
            });
        } else {
            if let Some(center_price) = market_view.get_mid_price(&symbol_to_trade) {
                let buy_bias = rng.gen_range(WHALE_PRICE_OFFSET_MIN..=WHALE_PRICE_OFFSET_MAX);
                let sell_bias = rng.gen_range(WHALE_PRICE_OFFSET_MIN..=WHALE_PRICE_OFFSET_MAX);
                let support_price = center_price.saturating_sub(buy_bias);
                let resistance_price = center_price.saturating_add(sell_bias);

                requests.push(OrderRequest::LimitOrder {
                    symbol: symbol_to_trade.clone(),
                    agent_id: self.id, side: Side::Buy, price: support_price, volume: WHALE_ORDER_VOLUME,
                });
                requests.push(OrderRequest::LimitOrder {
                    symbol: symbol_to_trade,
                    agent_id: self.id, side: Side::Sell, price: resistance_price, volume: WHALE_ORDER_VOLUME,
                });
            }
        }
        requests
    }

    fn buy_stock(&mut self, volume: u64, symbol: &Symbol) -> Vec<OrderRequest> {
        vec![OrderRequest::MarketOrder {
            symbol: symbol.clone(), agent_id: self.id, side: Side::Buy, volume,
        }]
    }

    fn sell_stock(&mut self, volume: u64, symbol: &Symbol) -> Vec<OrderRequest> {
        vec![OrderRequest::MarketOrder {
            symbol: symbol.clone(), agent_id: self.id, side: Side::Sell, volume,
        }]
    }

    fn margin_call(&mut self) -> Vec<OrderRequest> {
        if self.cash < -self.margin {
            let to_liquidate: Vec<(Symbol, i64)> = self.get_inventory()
                .iter()
                .map(|(s, &a)| (s.clone(), a))
                .collect();

            let mut requests = Vec::new();
            for (symbol, amount) in to_liquidate {
                if amount > 0 { requests.extend(self.sell_stock(amount.unsigned_abs(), &symbol)); } 
                else if amount < 0 { requests.extend(self.buy_stock(amount.unsigned_abs(), &symbol)); }
            }
            if !requests.is_empty() { println!("Liquidation for Whale agent {}!", self.id); }
            return requests;
        }
        vec![]
    }

    fn acknowledge_order(&mut self, order: Order) {
        self.open_orders.insert(order.id, order);
    }

    fn update_portfolio(&mut self, trade_volume: i64, trade: &Trade) {
        let inventory_for_symbol = self.inventory.entry(trade.symbol.clone()).or_insert(0);
        *inventory_for_symbol += trade_volume;

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
        Box::new(WhaleAgent::new(self.id))
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