// src/market.rs

// FIXED: Corrected the path from `stocks` to `stock`.
use crate::stocks::definitions::{Stock, Symbol};
use crate::stocks::registry;
use crate::{
    Agent, AgentType, DumbAgent, DumbLimitAgent, IpoAgent, MarketMakerAgent, MarketView,
    Marketable, Order, OrderBook, OrderRequest, Side, Trade, WhaleAgent,
};
use std::any::Any;
use std::collections::HashMap;

/// This is the main simulation engine. It owns the world state and participants.
pub struct Market {
    order_books: HashMap<Symbol, OrderBook>,
    agents: HashMap<usize, Box<dyn Agent>>,
    last_traded_prices: HashMap<Symbol, f64>,
    cumulative_volumes: HashMap<Symbol, u64>,
    initial_agent_types: Vec<AgentType>,
    order_id_counter: u64,
}

impl Market {
    pub fn new(participant_types: &[AgentType]) -> Self {
        let mut agents = HashMap::new();
        let mut agent_id_counter: usize = 0;

        for agent_type in participant_types {
            let agent = Self::create_agent_from_type(*agent_type, agent_id_counter);
            agents.insert(agent_id_counter, agent);
            agent_id_counter += 1;
        }

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

    pub fn cumulative_volume(&self, symbol: &Symbol) -> u64 {
        *self.cumulative_volumes.get(symbol).unwrap_or(&0)
    }

    pub fn get_total_inventory(&self, symbol: &Symbol) -> i64 {
        self.agents
            .values()
            .map(|agent| agent.get_inventory().get(symbol).cloned().unwrap_or(0))
            .sum()
    }
}

impl Marketable for Market {
    fn step(&mut self) -> f64 {
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

        for request in all_requests {
            // Get the symbol from the request first.
            let symbol = match &request {
                OrderRequest::LimitOrder { symbol, .. } => symbol.clone(),
                OrderRequest::MarketOrder { symbol, .. } => symbol.clone(),
                OrderRequest::CancelOrder { symbol, .. } => symbol.clone(),
            };

            // Process the request for that symbol.
            if let Some(book) = self.order_books.get_mut(&symbol) {
                match request {
                    OrderRequest::LimitOrder { agent_id, side, price, volume, symbol } => {
                        let mut order = Order {
                            symbol, id: self.next_order_id(), agent_id, side, price, volume, filled: 0,
                        };
                        // FIXED: Clone the order before moving it into acknowledge_order
                        if let Some(agent) = self.agents.get_mut(&agent_id) {
                            agent.acknowledge_order(order.clone());
                        }
                        trades_this_tick.extend(book.process_limit_order(&mut order));
                    }
                    OrderRequest::MarketOrder { agent_id, side, volume, symbol } => {
                        let last_price = self.last_traded_prices.get(&symbol).cloned().unwrap_or(0.0);
                        let order = Order {
                            symbol: symbol.clone(), id: self.next_order_id(), agent_id, side,
                            price: (last_price * 100.0).round() as u64,
                            volume, filled: 0,
                        };
                        if let Some(agent) = self.agents.get_mut(&agent_id) {
                            agent.acknowledge_order(order);
                        }
                        trades_this_tick.extend(book.process_market_order(agent_id, side, volume, &symbol));
                    }
                    OrderRequest::CancelOrder { agent_id, order_id, .. } => {
                        book.cancel_order(order_id, agent_id);
                    }
                }
            }
        }

        // --- Phase 3: Margin Call Phase ---
        let mut margin_requests = Vec::new();
        for id in &agent_ids {
            if let Some(agent) = self.agents.get_mut(id) {
                margin_requests.extend(agent.margin_call());
            }
        }
        for request in margin_requests {
             if let OrderRequest::MarketOrder { agent_id, side, volume, symbol } = request {
                 if let Some(book) = self.order_books.get_mut(&symbol) {
                    trades_this_tick.extend(book.process_market_order(agent_id, side, volume, &symbol));
                 }
             }
        }

        // --- Phase 4: Update Portfolios ---
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
        for trade in &trades_this_tick {
            self.last_traded_prices.insert(trade.symbol.clone(), trade.price as f64 / 100.0);
            let volume_entry = self.cumulative_volumes.entry(trade.symbol.clone()).or_insert(0);
            *volume_entry += trade.volume;
        }
        
        self.last_traded_prices.values().next().cloned().unwrap_or(0.0)
    }

    fn current_price(&self, symbol: &Symbol) -> Option<f64> {
        self.last_traded_prices.get(symbol).cloned()
    }
    
    // FIXED: Correctly implement the trait methods.
    fn get_order_book(&self, symbol: &Symbol) -> Option<&OrderBook> {
        self.order_books.get(symbol)
    }

    fn get_order_books(&self) -> &HashMap<Symbol, OrderBook> {
        &self.order_books
    }

    fn reset(&mut self) {
        // ... (reset logic is correct)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

// ... (tests will need updates but let's focus on compiling first)
