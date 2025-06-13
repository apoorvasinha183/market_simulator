// src/market.rs

use crate::{
    Agent, AgentType, DumbAgent, DumbLimitAgent, IpoAgent, MarketMakerAgent, MarketView, Marketable,
    Order, OrderBook, OrderRequest, Side, Trade, WhaleAgent,
};
use std::any::Any;
use std::collections::HashMap;

/// This is the main simulation engine. It owns the world state and participants.
pub struct Market {
    order_book: OrderBook,
    agents: HashMap<usize, Box<dyn Agent>>,
    last_traded_price: f64,
    initial_agent_types: Vec<AgentType>,
    order_id_counter: u64,
    cumulative_volume: u64,
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

        Self {
            order_book: OrderBook::new(),
            agents,
            last_traded_price: 150.00,
            initial_agent_types: participant_types.to_vec(),
            order_id_counter: 0,
            cumulative_volume: 0,
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

    pub fn get_order_book(&self) -> &OrderBook {
        &self.order_book
    }
    pub fn cumulative_volume(&self) -> u64 { self.cumulative_volume }
    pub fn get_total_inventory(&self) -> i64 {
        self.agents.values().map(|agent| agent.get_inventory()).sum()
    }
}

impl Marketable for Market {
    fn step(&mut self) -> f64 {
        // --- Phase 1: Agent Decisions ---
        let market_view = MarketView {
            order_book: &self.order_book,
            //last_traded_price: self.last_traded_price,
        };
        let mut all_requests = Vec::new();
        let agent_ids: Vec<usize> = self.agents.keys().cloned().collect();
        for id in &agent_ids {
            if let Some(agent) = self.agents.get_mut(id) {
                all_requests.extend(agent.decide_actions(&market_view));
            }
        }
        
        let mut trades_this_tick: Vec<Trade> = Vec::new();

        // --- Phase 2: Process All Requests ---
        for request in all_requests {
            match request {
                OrderRequest::LimitOrder { agent_id, side, price, volume } => {
                    let mut order = Order { id: self.next_order_id(), agent_id, side, price, volume, filled: 0 };
                    if let Some(agent) = self.agents.get_mut(&agent_id) {
                        agent.acknowledge_order(order);
                    }
                    trades_this_tick.extend(self.order_book.process_limit_order(&mut order));
                }
                OrderRequest::MarketOrder { agent_id, side, volume } => {
                     let mut order = Order { 
                        id: self.next_order_id(), agent_id, side, 
                        price: (self.last_traded_price * 100.0).round() as u64, // Price is notional for market order
                        volume, filled: 0 
                    };
                    if let Some(agent) = self.agents.get_mut(&agent_id) {
                        agent.acknowledge_order(order);
                    }
                    trades_this_tick.extend(self.order_book.process_market_order(agent_id, side, volume));
                }
                // --- NEW: Handle Cancel Requests ---
                OrderRequest::CancelOrder { agent_id, order_id } => {
                    self.order_book.cancel_order(order_id, agent_id);
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
             if let OrderRequest::MarketOrder { agent_id, side, volume } = request {
                trades_this_tick.extend(self.order_book.process_market_order(agent_id, side, volume));
            }
        }

        // --- Phase 4: Update Portfolios from all trades this tick ---
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
        if let Some(last_trade) = trades_this_tick.last() {
            self.last_traded_price = last_trade.price as f64 / 100.0;
        }
        let tick_volume: u64 = trades_this_tick.iter().map(|t| t.volume).sum();
        self.cumulative_volume = self.cumulative_volume.saturating_add(tick_volume);
        
        self.last_traded_price
    }

    fn current_price(&self) -> f64 {
        self.last_traded_price
    }
    
    fn reset(&mut self) {
        let mut agents = HashMap::new();
        for (id, agent_type) in self.initial_agent_types.iter().enumerate() {
            let agent = Self::create_agent_from_type(*agent_type, id);
            agents.insert(id, agent);
        }
        self.agents = agents;
        self.order_book = OrderBook::new();
        self.last_traded_price = 150.00;
        self.order_id_counter = 0;
        self.cumulative_volume = 0;
    }

    fn get_order_book(&self) -> Option<&OrderBook> {
        Some(&self.order_book)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
