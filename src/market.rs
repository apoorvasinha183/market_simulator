// src/market.rs

// We import all the necessary types here. The visualizer will not need to.
use crate::{
    Agent, AgentType, DumbAgent, DumbLimitAgent, Marketable, MarketView, OrderBook, OrderRequest,
    Side, Trade,
};
use std::any::Any;

/// This is the main simulation engine. It owns the world state (the order book)
/// and the participants (the agents), and runs the interaction loop.
pub struct Market {
    order_book: OrderBook,
    agents: Vec<Box<dyn Agent>>,
    last_traded_price: f64,
    // Store the initial agent configuration to use for resetting.
    initial_agent_types: Vec<AgentType>,
}

impl Market {
    /// The one true public constructor. It takes the desired agent types and builds the market.
    pub fn new(participant_types: &[AgentType]) -> Self {
        let mut agents: Vec<Box<dyn Agent>> = Vec::new();
        let mut agent_id_counter: usize = 0;

        // Loop through the requested agent types and create ONE instance of each.
        for agent_type in participant_types {
            let agent = Self::create_agent_from_type(*agent_type, agent_id_counter);
            agents.push(agent);
            agent_id_counter += 1;
        }

        Self {
            order_book: OrderBook::new_random(),
            agents,
            last_traded_price: 150.00,
            // Store the configuration for use in the reset method.
            initial_agent_types: participant_types.to_vec(),
        }
    }

    // Private helper to create an agent from an enum variant.
    fn create_agent_from_type(agent_type: AgentType, id: usize) -> Box<dyn Agent> {
        match agent_type {
            AgentType::DumbMarket => Box::new(DumbAgent::new(id)),
            AgentType::DumbLimit => Box::new(DumbLimitAgent::new(id)),
        }
    }

    /// A public getter for the visualizer to inspect the current order book state.
    pub fn get_order_book(&self) -> &OrderBook {
        &self.order_book
    }
}

// This implementation of the Marketable trait is now correct.
impl Marketable for Market {
    /// This is the core "tick" of the simulation.
    fn step(&mut self) -> f64 {
        let market_view = MarketView {
            order_book: &self.order_book,
        };

        let mut all_requests = Vec::new();
        for agent in self.agents.iter_mut() {
            all_requests.extend(agent.decide_actions(&market_view));
        }

        let mut trades_this_tick: Vec<Trade> = Vec::new();
        for request in all_requests {
            match request {
                OrderRequest::MarketOrder {
                    agent_id: _,
                    side,
                    volume,
                } => {
                    trades_this_tick.extend(self.order_book.process_market_order(side, volume));
                }
                OrderRequest::LimitOrder {
                    agent_id: _,
                    side,
                    price,
                    volume,
                } => {
                    self.order_book.add_limit_order(price, volume, side);
                }
            }
        }

        if let Some(last_trade) = trades_this_tick.last() {
            self.last_traded_price = last_trade.price as f64 / 100.0;
        }

        self.last_traded_price
    }

    fn current_price(&self) -> f64 {
        self.last_traded_price
    }

    /// Resets the market by creating a new order book and re-initializing the agent population
    /// based on the stored initial configuration.
    fn reset(&mut self) {
        self.order_book = OrderBook::new_random();
        let mut new_agents: Vec<Box<dyn Agent>> = Vec::new();
        for (id, agent_type) in self.initial_agent_types.iter().enumerate() {
            new_agents.push(Self::create_agent_from_type(*agent_type, id));
        }
        self.agents = new_agents;
        self.last_traded_price = 150.00;
    }

    fn get_order_book(&self) -> Option<&OrderBook> {
        Some(&self.order_book)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
    