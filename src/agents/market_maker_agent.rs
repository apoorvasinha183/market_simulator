// src/agents/market_maker_agent.rs

use super::agent_trait::Agent;
use super::config::{
    MARKET_MAKER_ACTION_PROB, MARKET_MAKER_SPREAD, MARKET_MAKER_VOL_MAX, MARKET_MAKER_VOL_MIN,
};
use crate::agents::latency::MARKET_MAKER_TICKS_UNTIL_ACTIVE;
// FIXED: Use the top-level re-exported types.
use crate::{MarketView, OrderBook, Order, OrderRequest, Side, Trade};
use crate::stocks::definitions::Symbol;
use rand::Rng;
use std::collections::HashMap;

pub struct MarketMakerAgent {
    pub id: usize,
    inventory: HashMap<Symbol, i64>,
    ticks_until_active: u32,
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
            ticks_until_active: MARKET_MAKER_TICKS_UNTIL_ACTIVE,
            open_orders: HashMap::new(),
            cash: 100_000_000.0,
            margin: 10_000_000_000.0,
            port_value: 0.0,
        }
    }

    fn place_quotes(&self, book: &OrderBook, symbol: &Symbol) -> Vec<OrderRequest> {
        // FIXED: Use public getter method instead of private field.
        let center_price = book.get_bids().keys().last().cloned().unwrap_or(15000);

        let bid_price = center_price - MARKET_MAKER_SPREAD;
        let ask_price = center_price + MARKET_MAKER_SPREAD;

        let mut rng = rand::thread_rng();
        let volume = rng.gen_range(MARKET_MAKER_VOL_MIN..=MARKET_MAKER_VOL_MAX);

        let bid_order = OrderRequest::LimitOrder {
            symbol: symbol.clone(),
            agent_id: self.id,
            side: Side::Buy,
            price: bid_price,
            volume,
        };

        let ask_order = OrderRequest::LimitOrder {
            symbol: symbol.clone(),
            agent_id: self.id,
            side: Side::Sell,
            price: ask_price,
            volume,
        };
        vec![bid_order, ask_order]
    }

    fn check_inventory(&self, _market_view: &MarketView) -> Vec<OrderRequest> {
        vec![]
    }

    fn get_spread(&self, order_book: &OrderBook) -> Option<u64> {
        // FIXED: Use public getter methods instead of private fields.
        let best_bid = order_book.get_bids().keys().last().cloned();
        let best_ask = order_book.get_asks().keys().next().cloned();

        if let (Some(bid), Some(ask)) = (best_bid, best_ask) {
            Some(ask - bid)
        } else {
            None
        }
    }
}

impl Agent for MarketMakerAgent {
    fn decide_actions(&mut self, market_view: &MarketView) -> Vec<OrderRequest> {
        if self.ticks_until_active > 0 {
            self.ticks_until_active -= 1;
            return vec![];
        }
        let mut rng = rand::thread_rng();
        let mut requests = Vec::new();
        if rng.gen_bool(MARKET_MAKER_ACTION_PROB as f64) {
            if let Some((symbol, book)) = market_view.order_books.iter().next() {
                if let Some(spread) = self.get_spread(book) {
                    if spread > 1 {
                        requests.extend(self.place_quotes(book, symbol));
                    }
                } else {
                    requests.extend(self.place_quotes(book, symbol));
                }
            }
        }
        requests.extend(self.check_inventory(market_view));
        requests
    }
    
    fn buy_stock(&mut self, _volume: u64, _symbol: &Symbol) -> Vec<OrderRequest> {
        vec![]
    }
    fn sell_stock(&mut self, _volume: u64, _symbol: &Symbol) -> Vec<OrderRequest> {
        vec![]
    }
    fn margin_call(&mut self) -> Vec<OrderRequest> {
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
            if let Some(ord) = self.open_orders.get_mut(&trade.maker_order_id) {
                ord.filled += trade.volume;
                if ord.filled >= ord.volume {
                    self.open_orders.remove(&trade.maker_order_id);
                }
            }
        }
    }
    fn get_pending_orders(&self) -> Vec<Order> {
        self.open_orders.values().cloned().collect()
    }
    fn cancel_open_order(&mut self, _order_id: u64) -> Vec<OrderRequest> {
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