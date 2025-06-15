// src/agents/dumb_limit_agent.rs

// FIXED: Corrected the use path from `stocks` to `stock`.
use crate::stocks::definitions::Symbol;
use super::agent_trait::{Agent, MarketView};
use super::config::{
    LIMIT_AGENT_ACTION_PROB, LIMIT_AGENT_MAX_OFFSET, LIMIT_AGENT_NUM_TRADERS,
    LIMIT_AGENT_VOL_MAX, LIMIT_AGENT_VOL_MIN,
};
use crate::agents::latency::LIMIT_AGENT_TICKS_UNTIL_ACTIVE;
use crate::simulators::order_book::Trade;
use crate::types::order::{Order, OrderRequest, Side};
use rand::Rng;
use std::collections::HashMap;

pub struct DumbLimitAgent {
    pub id: usize,
    inventory: HashMap<Symbol, i64>,
    ticks_until_active: u32,
    open_orders: HashMap<u64, Order>,
    cash: f64,
    margin: f64,
    port_value: f64,
}

impl DumbLimitAgent {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            inventory: HashMap::new(),
            ticks_until_active: LIMIT_AGENT_TICKS_UNTIL_ACTIVE,
            open_orders: HashMap::new(),
            cash: 100_000_000.0,
            margin: 10_000_000_000.0,
            port_value: 0.0,
        }
    }
}

impl Agent for DumbLimitAgent {
    fn decide_actions(&mut self, market_view: &MarketView) -> Vec<OrderRequest> {
        if self.ticks_until_active > 0 {
            self.ticks_until_active -= 1;
            return vec![];
        }

        let mut rng = rand::thread_rng();
        let mut requests = Vec::new();

        let Some(symbol_to_trade) = market_view.order_books.keys().next().cloned() else {
            return vec![];
        };

        for _ in 0..LIMIT_AGENT_NUM_TRADERS {
            if rng.gen_bool(LIMIT_AGENT_ACTION_PROB as f64) {
                if let Some(mid_price) = market_view.get_mid_price(&symbol_to_trade) {
                    let side = if rng.gen_bool(0.5) { Side::Buy } else { Side::Sell };
                    let offset = rng.gen_range(1..=LIMIT_AGENT_MAX_OFFSET);
                    let price = match side {
                        Side::Buy => mid_price.saturating_sub(offset),
                        Side::Sell => mid_price.saturating_add(offset),
                    };

                    let volume = rng.gen_range(LIMIT_AGENT_VOL_MIN..=LIMIT_AGENT_VOL_MAX);

                    if side == Side::Buy {
                        let estimated_cost = (volume as f64) * (price as f64 / 100.0);
                        if estimated_cost > self.cash + self.margin {
                            continue;
                        }
                    }

                    requests.push(OrderRequest::LimitOrder {
                        symbol: symbol_to_trade.clone(),
                        agent_id: self.id,
                        side,
                        price,
                        volume,
                    });
                }
            }
        }
        requests
    }

    fn buy_stock(&mut self, volume: u64, symbol: &Symbol) -> Vec<OrderRequest> {
        vec![OrderRequest::MarketOrder {
            symbol: symbol.clone(),
            agent_id: self.id,
            side: Side::Buy,
            volume,
        }]
    }

    fn sell_stock(&mut self, volume: u64, symbol: &Symbol) -> Vec<OrderRequest> {
        vec![OrderRequest::MarketOrder {
            symbol: symbol.clone(),
            agent_id: self.id,
            side: Side::Sell,
            volume,
        }]
    }

    // FIXED: Corrected the borrow checker error in margin call logic.
    fn margin_call(&mut self) -> Vec<OrderRequest> {
        if self.cash < -self.margin {
            let to_liquidate: Vec<(Symbol, i64)> = self.get_inventory()
                .iter()
                .filter(|(_, &amount)| amount > 0)
                .map(|(symbol, &amount)| (symbol.clone(), amount))
                .collect();
            
            let mut requests = Vec::new();
            for (symbol, amount) in to_liquidate {
                requests.extend(self.sell_stock(amount as u64, &symbol));
            }

            if !requests.is_empty() {
                println!("Liquidation for agent {}!", self.id);
            }
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
        // A full implementation would need to look up the order, get its symbol,
        // and then create a CancelOrder request.
        vec![]
    }

    fn get_id(&self) -> usize {
        self.id
    }

    fn get_inventory(&self) -> &HashMap<Symbol, i64> {
        &self.inventory
    }

    fn clone_agent(&self) -> Box<dyn Agent> {
        Box::new(DumbLimitAgent::new(self.id))
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
