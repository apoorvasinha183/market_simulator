// src/market.rs

// NEW: We will need these to initialize the market and to use Symbol as a key
use crate::stocks::definitions::{Stock,Symbol};
use crate::stocks::registry;
use crate::{
    Agent, AgentType, DumbAgent, DumbLimitAgent, IpoAgent, MarketMakerAgent, MarketView,
    Marketable, Order, OrderBook, OrderRequest, Side, Trade, WhaleAgent,
};
use std::any::Any;
use std::collections::HashMap;

/// This is the main simulation engine. It owns the world state and participants.
// CHANGED: The Market struct now holds HashMaps for per-symbol data.
pub struct Market {
    // A separate order book for each symbol.
    order_books: HashMap<Symbol, OrderBook>,
    // A map of all agents participating in the market.
    agents: HashMap<usize, Box<dyn Agent>>,
    // The last traded price for each symbol.
    last_traded_prices: HashMap<Symbol, f64>,
    // The total cumulative volume traded for each symbol.
    cumulative_volumes: HashMap<Symbol, u64>,
    // The initial configuration of agent types for resetting the simulation.
    initial_agent_types: Vec<AgentType>,
    // A single counter for generating unique order IDs across all markets.
    order_id_counter: u64,
}

impl Market {
    // CHANGED: The constructor now builds the market from the stock registry.
    pub fn new(participant_types: &[AgentType]) -> Self {
        let mut agents = HashMap::new();
        let mut agent_id_counter: usize = 0;

        for agent_type in participant_types {
            let agent = Self::create_agent_from_type(*agent_type, agent_id_counter);
            agents.insert(agent_id_counter, agent);
            agent_id_counter += 1;
        }

        // NEW: Initialize the market for all stocks defined in the registry.
        let stocks = registry::get_tradable_universe();
        let mut order_books = HashMap::new();
        let mut last_traded_prices = HashMap::new();
        let mut cumulative_volumes = HashMap::new();

        for stock in stocks {
            order_books.insert(stock.symbol.clone(), OrderBook::new());
            last_traded_prices.insert(stock.symbol.clone(), stock.initial_price);
            cumulative_volumes.insert(stock.symbol.clone(), 0);
        }

        Self {
            order_books,
            agents,
            last_traded_prices,
            cumulative_volumes,
            initial_agent_types: participant_types.to_vec(),
            order_id_counter: 0,
        }
    }

    fn create_agent_from_type(agent_type: AgentType, id: usize) -> Box<dyn Agent> {
        match agent_type {
            AgentType::DumbMarket => Box::new(DumbAgent::new(id)),
            AgentType::DumbLimit => Box::new(DumbLimitAgent::new(id)),
            AgentType::MarketMaker => Box::new(MarketMakerAgent::new(id)),
            AgentType::IPO => Box::new(IpoAgent::new(id)),
            AgentType::WhaleAgent => Box::new(WhaleAgent::new(id)),
        }
    }

    fn next_order_id(&mut self) -> u64 {
        self.order_id_counter += 1;
        self.order_id_counter
    }

    // CHANGED: This function now needs a symbol to know which order book to return.
    pub fn get_order_book(&self, symbol: &Symbol) -> Option<&OrderBook> {
        self.order_books.get(symbol)
    }

    // CHANGED: This function now needs a symbol to know which volume to return.
    pub fn cumulative_volume(&self, symbol: &Symbol) -> u64 {
        *self.cumulative_volumes.get(symbol).unwrap_or(&0)
    }

    // CHANGED: This function now needs a symbol to get the relevant inventory.
    // NOTE: This will require changing get_inventory() on the Agent trait.
    pub fn get_total_inventory(&self, symbol: &Symbol) -> i64 {
        self.agents
            .values()
            .map(|agent| agent.get_inventory().get(symbol).cloned().unwrap_or(0))
            .sum()
    }
}

impl Marketable for Market {
    // CHANGED: The step function now orchestrates trades across multiple order books.
    fn step(&mut self) -> f64 {
        // --- Phase 1: Agent Decisions ---
        // The MarketView now needs to provide access to all order books.
        let market_view = MarketView {
            order_books: &self.order_books,
            last_traded_prices: &self.last_traded_prices,
        };
        let mut all_requests = Vec::new();
        let mut agent_ids: Vec<usize> = self.agents.keys().cloned().collect();
        agent_ids.sort_unstable();
        for id in &agent_ids {
            if let Some(agent) = self.agents.get_mut(id) {
                all_requests.extend(agent.decide_actions(&market_view));
            }
        }

        let mut trades_this_tick: Vec<Trade> = Vec::new();

        // --- Phase 2: Process All Requests ---
        for request in all_requests {
            // NEW: The core change. We route the order to the correct order book
            // based on the symbol in the request.
            let (symbol, order_book) = match &request {
                OrderRequest::LimitOrder { symbol, .. } => (symbol, self.order_books.get_mut(symbol)),
                OrderRequest::MarketOrder { symbol, .. } => (symbol, self.order_books.get_mut(symbol)),
                OrderRequest::CancelOrder { symbol, .. } => (symbol, self.order_books.get_mut(symbol)),
            };

            // Only proceed if the symbol is valid and we have an order book for it.
            if let Some(book) = order_book {
                match request {
                    OrderRequest::LimitOrder { agent_id, side, price, volume, .. } => {
                        let mut order = Order {
                            symbol: symbol.clone(),
                            id: self.next_order_id(),
                            agent_id, side, price, volume, filled: 0,
                        };
                        if let Some(agent) = self.agents.get_mut(&agent_id) {
                            agent.acknowledge_order(order);
                        }
                        trades_this_tick.extend(book.process_limit_order(&mut order));
                    }
                    OrderRequest::MarketOrder { agent_id, side, volume, .. } => {
                        let last_price = self.last_traded_prices.get(symbol).cloned().unwrap_or(0.0);
                        let order = Order {
                            symbol: symbol.clone(),
                            id: self.next_order_id(),
                            agent_id, side,
                            price: (last_price * 100.0).round() as u64,
                            volume, filled: 0,
                        };
                        if let Some(agent) = self.agents.get_mut(&agent_id) {
                            agent.acknowledge_order(order);
                        }
                        trades_this_tick.extend(book.process_market_order(agent_id, side, volume));
                    }
                    OrderRequest::CancelOrder { agent_id, order_id, .. } => {
                        book.cancel_order(order_id, agent_id);
                    }
                }
            }
        }

        // --- Phase 3: Margin Call Phase (Will reimplement it later) ---
        

        // --- Phase 4: Update Portfolios from all trades this tick ---
        // This loop is now symbol-aware because the `Trade` object contains the symbol.
        for trade in &trades_this_tick {
            if let Some(taker) = self.agents.get_mut(&trade.taker_agent_id) {
                let change = if trade.taker_side == Side::Buy { trade.volume as i64 } else { -(trade.volume as i64) };
                taker.update_portfolio(change, trade);
            }
            if let Some(maker) = self.agents.get_mut(&trade.maker_agent_id) {
                let change = if trade.taker_side == Side::Sell { trade.volume as i64 } else { -(trade.volume as i64) };
                maker.update_portfolio(change, trade);
            }
        }

        // --- Phase 5: Update Market-Level State ---
        // NEW: Update prices and volumes for each symbol that had a trade.
        for trade in &trades_this_tick {
            self.last_traded_prices.insert(trade.symbol.clone(), trade.price as f64 / 100.0);
            let volume_entry = self.cumulative_volumes.entry(trade.symbol.clone()).or_insert(0);
            *volume_entry += trade.volume;
        }
        
        // The return value is now ambiguous. For now, we return the price of the first symbol.
        // This will need to be addressed in the consuming UI code.
        self.last_traded_prices.values().next().cloned().unwrap_or(0.0)
    }

    // CHANGED: This is now ambiguous. Let's return the price of the first available symbol.
    fn current_price(&self) -> f64 {
        self.last_traded_prices.values().next().cloned().unwrap_or(0.0)
    }

    // CHANGED: Reset needs to re-initialize all order books.
    fn reset(&mut self) {
        let mut agents = HashMap::new();
        for (id, agent_type) in self.initial_agent_types.iter().enumerate() {
            let agent = Self::create_agent_from_type(*agent_type, id);
            agents.insert(id, agent);
        }
        self.agents = agents;

        let stocks = registry::get_tradable_universe();
        self.order_books.clear();
        self.last_traded_prices.clear();
        self.cumulative_volumes.clear();

        for stock in stocks {
            self.order_books.insert(stock.symbol.clone(), OrderBook::new());
            self.last_traded_prices.insert(stock.symbol.clone(), stock.initial_price);
            self.cumulative_volumes.insert(stock.symbol.clone(), 0);
        }
        
        self.order_id_counter = 0;
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

// NOTE: The tests below will fail to compile until you update the Order,
// OrderRequest, and Agent structs to be symbol-aware.
// I have updated them assuming these changes will be made.
#[cfg(test)]
mod tests {
    use super::*;

    /// A controllable agent for testing specific scenarios.
    #[derive(Default)]
    struct TestAgent {
        id: usize,
        inventory: HashMap<Symbol, i64>, // CHANGED
        requests: Vec<OrderRequest>,
        acknowledged_orders: HashMap<u64, Order>,
    }

    impl Agent for TestAgent {
        fn decide_actions(&mut self, _market_view: &MarketView) -> Vec<OrderRequest> {
            self.requests.drain(..).collect()
        }
        fn acknowledge_order(&mut self, order: Order) {
            self.acknowledged_orders.insert(order.id, order);
        }
        // CHANGED
        fn update_portfolio(&mut self, change: i64, trade: &Trade) {
            *self.inventory.entry(trade.symbol.clone()).or_insert(0) += change;
        }
        // CHANGED
        fn get_inventory(&self) -> &HashMap<Symbol, i64> {
            &self.inventory
        }

        fn get_pending_orders(&self) -> Vec<Order> {
            self.acknowledged_orders.values().cloned().collect()
        }
        fn margin_call(&mut self) -> Vec<OrderRequest> { vec![] }
        fn buy_stock(&mut self, _vol: u64, _symbol: &Symbol) -> Vec<OrderRequest> { vec![] }
        fn sell_stock(&mut self, _vol: u64, _symbol: &Symbol) -> Vec<OrderRequest> { vec![] }
        fn cancel_open_order(&mut self, _id: u64) -> Vec<OrderRequest> { vec![] }
        fn get_id(&self) -> usize { self.id }
        fn clone_agent(&self) -> Box<dyn Agent> { Box::new(Self::default()) }
        fn evaluate_port(&mut self, _market_view: &MarketView) -> f64 { 0.0 }
    }

    #[test]
    fn test_end_to_end_trade_and_inventory_update() {
        // Arrange
        let agent_types = vec![]; 
        let mut market = Market::new(&agent_types);
        let symbol = "GEM".to_string();

        let mut seller = Box::new(TestAgent::default());
        seller.inventory.insert(symbol.clone(), 100);
        seller.requests.push(OrderRequest::LimitOrder {
            symbol: symbol.clone(), // NEW
            agent_id: 0, side: Side::Sell, price: 15000, volume: 50,
        });

        let mut buyer = Box::new(TestAgent::default());
        buyer.requests.push(OrderRequest::MarketOrder {
            symbol: symbol.clone(), // NEW
            agent_id: 1, side: Side::Buy, volume: 30,
        });

        market.agents.insert(0, seller);
        market.agents.insert(1, buyer);

        // Act
        market.step();

        // Assert
        let seller_final = market.agents.get(&0).unwrap();
        let buyer_final = market.agents.get(&1).unwrap();

        assert_eq!(*seller_final.get_inventory().get(&symbol).unwrap(), 70);
        assert_eq!(*buyer_final.get_inventory().get(&symbol).unwrap(), 30);
        assert_eq!(*market.last_traded_prices.get(&symbol).unwrap(), 150.00);
    }
}
