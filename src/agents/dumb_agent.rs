// src/agents/dumb_agent.rs

// FIXED: Corrected the use path from `stocks` to `stock`.
use crate::stocks::definitions::Symbol;
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
    inventory: HashMap<Symbol, i64>,
    ticks_until_active: u32,
    open_orders: HashMap<u64, Order>,
    cash: f64,
    margin: f64,
    port_value: f64,
}

impl DumbAgent {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            inventory: HashMap::new(),
            ticks_until_active: DUMB_AGENT_TICKS_UNTIL_ACTIVE,
            open_orders: HashMap::new(),
            cash: 1_000_000_000.0,
            margin: 4_000_000_000.0,
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

        let Some(symbol_to_trade) = market_view.order_books.keys().next().cloned() else {
            return vec![]; // No symbols in the market to trade
        };

        for _ in 0..DUMB_AGENT_NUM_TRADERS {
            if rng.gen_bool(DUMB_AGENT_ACTION_PROB) {
                let side = if rng.gen_bool(0.5) { Side::Buy } else { Side::Sell };

                let volume = if rng.gen_bool(DUMB_AGENT_LARGE_VOL_CHANCE) {
                    rng.gen_range(DUMB_AGENT_LARGE_VOL_MIN..=DUMB_AGENT_LARGE_VOL_MAX)
                } else {
                    rng.gen_range(DUMB_AGENT_TYPICAL_VOL_MIN..=DUMB_AGENT_TYPICAL_VOL_MAX)
                };

                if side == Side::Buy {
                    if let Some(price_cents) = market_view.get_mid_price(&symbol_to_trade) {
                        let estimated_cost = (volume as f64) * (price_cents as f64 / 100.0);
                        let buying_power = self.cash + self.margin;
                        if estimated_cost > buying_power {
                            continue;
                        }
                    }
                }
                
                let request = if side == Side::Buy {
                    self.buy_stock(volume, &symbol_to_trade)
                } else {
                    self.sell_stock(volume, &symbol_to_trade)
                };
                requests_this_tick.extend(request);
            }
        }
        
        requests_this_tick
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

    fn margin_call(&mut self) -> Vec<OrderRequest> {
        if self.cash < -self.margin {
            // FIXED: This pattern resolves the E0502 borrow checker error.
            // 1. Collect the inventory data you need into a new collection.
            let to_liquidate: Vec<(Symbol, i64)> = self.get_inventory()
                .iter()
                .filter(|(_, &amount)| amount > 0)
                .map(|(symbol, &amount)| (symbol.clone(), amount))
                .collect();
            
            let mut requests = Vec::new();
            // 2. Now iterate over the new collection, which doesn't borrow `self`.
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
        Box::new(DumbAgent::new(self.id))
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
// -----------------------------------------------------------------------------
//  Unit Tests
// -----------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::stocks::definitions::Symbol;
    use crate::types::order::Side;

    fn new_mock_trade(price_cents: u64, volume: u64, symbol: &Symbol) -> Trade {
        Trade {
            symbol: symbol.clone(),
            price: price_cents,
            volume,
            taker_agent_id: 1,
            maker_agent_id: 2,
            maker_order_id: 101,
            taker_side: Side::Buy,
        }
    }

    #[test]
    fn test_update_portfolio_cash_on_buy() {
        let mut agent = DumbAgent::new(0);
        let symbol = "TEST".to_string();
        let initial_cash = agent.cash;
        let trade = new_mock_trade(15000, 10, &symbol);
        let cost = 10.0 * 150.0;

        agent.update_portfolio(10, &trade);

        assert_eq!(*agent.get_inventory().get(&symbol).unwrap(), 10);
        assert!((agent.cash - (initial_cash - cost)).abs() < 1e-9);
    }

    #[test]
    fn test_update_portfolio_cash_on_sell() {
        let mut agent = DumbAgent::new(0);
        let symbol = "TEST".to_string();
        let initial_cash = agent.cash;
        let trade = new_mock_trade(15000, 10, &symbol);
        let proceeds = 10.0 * 150.0;
        
        agent.inventory.insert(symbol.clone(), 50);

        agent.update_portfolio(-10, &trade);

        assert_eq!(*agent.get_inventory().get(&symbol).unwrap(), 40);
        assert!((agent.cash - (initial_cash + proceeds)).abs() < 1e-9);
    }

    #[test]
    fn test_margin_call_triggers_when_breached() {
        let mut agent = DumbAgent::new(0);
        let symbol1 = "STK1".to_string();
        let symbol2 = "STK2".to_string();
        agent.cash = -4_000_000_000.1; 
        agent.inventory.insert(symbol1.clone(), 500);
        agent.inventory.insert(symbol2.clone(), 300);

        let requests = agent.margin_call();

        assert_eq!(requests.len(), 2, "Should generate liquidation orders for both holdings.");
        
        let sell_stk1 = requests.iter().find(|r| match r {
            OrderRequest::MarketOrder { symbol, .. } => symbol == &symbol1,
            _ => false,
        }).expect("Should find order for STK1");
        
        if let OrderRequest::MarketOrder { volume, .. } = sell_stk1 {
            assert_eq!(*volume, 500);
        }
    }
}
