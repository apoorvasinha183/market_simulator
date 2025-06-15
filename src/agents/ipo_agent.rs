// src/agents/ipo_agent.rs

// NEW: We need the Symbol type for the HashMap.
use crate::stocks::definitions::Symbol;
use super::agent_trait::Agent;
// FIXED: Use the top-level re-exported types.
use crate::{MarketView, Order, OrderRequest, Side, Trade};
use std::collections::HashMap;
/// An agent that acts only once at the beginning of the simulation
/// to place the entire initial float of assets on the market.
pub struct IpoAgent {
    pub id: usize,
    // CHANGED: Inventory is now a HashMap to hold the float for each stock.
    inventory: HashMap<Symbol, i64>,
    has_acted: bool,
    open_orders: HashMap<u64, Order>,
    // NEW: Added financial fields for trait conformity.
    cash: f64,
}

impl IpoAgent {
    pub fn new(id: usize) -> Self {
        // NEW: The IPO agent now starts with a pre-configured inventory of stocks.
        // This would typically be loaded from the stock registry.
        // For now, we hardcode it to hold the float for two stocks.
        let mut inventory = HashMap::new();
        inventory.insert("AAPL".to_string(), 1_000_000_000);
        inventory.insert("QQQ".to_string(), 500_000_000);

        Self {
            id,
            inventory,
            has_acted: false,
            open_orders: HashMap::new(),
            // The IPO agent has effectively infinite cash/credit.
            cash: 0.0,
        }
    }
}

impl Agent for IpoAgent {
    // CHANGED: This logic now creates IPOs for all stocks in its inventory.
    fn decide_actions(&mut self, market_view: &MarketView) -> Vec<OrderRequest> {
        if self.has_acted {
            return vec![];
        }

        self.has_acted = true;
        println!("--- IPO AGENT IS ACTING ---");

        let mut orders = Vec::new();

        // Iterate through each stock the IPO agent owns.
        for (symbol, &total_float) in &self.inventory {
            // Use the stock's initial price from the market view to set the IPO price range.
            if let Some(initial_price) = market_view.last_traded_prices.get(symbol) {
                let num_price_levels = 20;
                let volume_per_level = (total_float / num_price_levels as i64) as u64;
                let start_price_cents = (initial_price * 100.0) as u64;
                let tick_size = 5; // $0.05 per tick

                println!(
                    "  -> Creating IPO for {} starting at ${:.2}",
                    symbol, initial_price
                );

                for i in 0..num_price_levels {
                    let price = start_price_cents + i * tick_size;
                    orders.push(OrderRequest::LimitOrder {
                        symbol: symbol.clone(), // NEW
                        agent_id: self.id,
                        side: Side::Sell,
                        price,
                        volume: volume_per_level,
                    });
                }
            }
        }
        orders
    }

    // CHANGED: Method signature updated to match the trait.
    fn buy_stock(&mut self, _volume: u64, _symbol: &Symbol) -> Vec<OrderRequest> {
        vec![] // The IPO agent only sells.
    }

    // CHANGED: Method signature updated to match the trait.
    fn sell_stock(&mut self, _volume: u64, _symbol: &Symbol) -> Vec<OrderRequest> {
        vec![] // Initial selling is handled in `decide_actions`.
    }

    fn margin_call(&mut self) -> Vec<OrderRequest> {
        vec![] // This agent is always long and cannot be margin called.
    }

    fn acknowledge_order(&mut self, order: Order) {
        self.open_orders.insert(order.id, order);
    }

    // CHANGED: Applying the correct accounting logic.
    fn update_portfolio(&mut self, trade_volume: i64, trade: &Trade) {
        // Update inventory for the correct symbol.
        let inventory_for_symbol = self.inventory.entry(trade.symbol.clone()).or_insert(0);
        *inventory_for_symbol += trade_volume;

        // The IPO agent receives cash from selling its initial float.
        let cash_change = (trade_volume as f64) * (trade.price as f64 / 100.0);
        self.cash -= cash_change;

        // Update open order status if this agent was the maker.
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
        if self.open_orders.remove(&order_id).is_some() {
            // A full implementation would create a CancelOrder request.
        }
        vec![]
    }

    fn get_id(&self) -> usize {
        self.id
    }

    // CHANGED: Method signature updated to match the trait.
    fn get_inventory(&self) -> &HashMap<Symbol, i64> {
        &self.inventory
    }

    fn clone_agent(&self) -> Box<dyn Agent> {
        Box::new(IpoAgent::new(self.id))
    }

    // CHANGED: Now evaluates the total value across all symbol holdings.
    fn evaluate_port(&mut self, market_view: &MarketView) -> f64 {
        let mut total_value = 0.0;
        for (symbol, &amount) in &self.inventory {
            if let Some(price_cents) = market_view.get_mid_price(symbol) {
                let value_cents = (amount as i128) * (price_cents as i128);
                total_value += (value_cents as f64) / 100.0;
            }
        }
        total_value
    }
}
