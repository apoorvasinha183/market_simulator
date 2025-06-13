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
}

impl WhaleAgent {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            inventory: WHALE_INITIAL_INVENTORY,
            ticks_until_active: WHALE_TICKS_UNTIL_ACTIVE,
            open_orders: HashMap::new(),
        }
    }
}

impl Agent for WhaleAgent {
    fn decide_actions(&mut self, market_view: &MarketView) -> Vec<OrderRequest> {
        if self.ticks_until_active > 0 {
            self.ticks_until_active -= 1;
            return vec![];
        }

        let mut rng = rand::thread_rng(); // Create rng instance once

        if !rng.gen_bool(WHALE_ACTION_PROB) {
            return vec![]; // The whale is patient.
        }

        // --- Cancel and Replace Logic ---
        println!("The whale is active! \n");
        
        let ids_to_cancel: Vec<u64> = self.open_orders.keys().cloned().collect();
        let mut requests: Vec<OrderRequest> = ids_to_cancel
            .into_iter()
            .flat_map(|id| self.cancel_open_order(id)) // This now returns real CancelOrder requests
            .collect();
        
        self.open_orders.clear(); // Agent assumes its cancels will succeed.

        // --- Place new orders ---
        if rng.gen_bool(CRAZY_WHALE) {
            println!("MARKET MANIPULATION!!! SEC SEC!");
            let crazy_volume = rng.gen_range((WHALE_ORDER_VOLUME / 2)..=WHALE_ORDER_VOLUME);
            println!("MARKET MANIPULATION!!! SEC SEC! {} shares are being manipulated! PEDRO", crazy_volume);
            
            let side = if rng.gen_bool(0.7) { Side::Buy } else { Side::Sell };
            requests.push(OrderRequest::MarketOrder { agent_id: self.id, side, volume: crazy_volume });

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
        
        requests
    }

    // --- Fulfillment of the Agent Trait Contract ---

    fn buy_stock(&mut self, _volume: u64) -> Vec<OrderRequest> { vec![] }
    fn sell_stock(&mut self, _volume: u64) -> Vec<OrderRequest> { vec![] }
    fn margin_call(&mut self) -> Vec<OrderRequest> { vec![] }

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

    /// This function now generates a real CancelOrder request.
    fn cancel_open_order(&mut self, order_id: u64) -> Vec<OrderRequest> {
        if self.open_orders.contains_key(&order_id) {
            return vec![OrderRequest::CancelOrder {
                agent_id: self.id,
                order_id,
            }];
        }
        vec![]
    }
    
    fn get_id(&self) -> usize { self.id }
    fn get_inventory(&self) -> i64 { self.inventory }
    fn clone_agent(&self) -> Box<dyn Agent> { Box::new(WhaleAgent::new(self.id)) }
}
