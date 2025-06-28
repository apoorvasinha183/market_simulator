// src/orchestra.rs

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::thread::{self, JoinHandle};

// --- Crate-level imports ---
use crate::OrderBook; // Assuming order_book.rs is in simulators
use crate::agents::agent_trait::Agent;
use crate::agents::agent_type::AgentType;
use crate::agents::dumb_agent::DumbAgent;
use crate::agents::dumb_limit_agent::DumbLimitAgent;
use crate::agents::ipo_agent::IpoAgent;
use crate::agents::market_maker_agent::MarketMakerAgent;
use crate::agents::whale_agent::WhaleAgent;
use crate::market::Market;
use crate::simulators::market_trait::Marketable;
use crate::stocks::StockMarket;
use crate::types::order::{Order, OrderRequest, Trade};
use crossbeam_channel::{Sender, unbounded};

// --- This is the shared state agents will read from. ---
#[derive(Debug, Clone)] // Clone is needed for the Market to initialize the shadow book
pub struct MarketState {
    pub order_books: HashMap<u64, OrderBook>,
    pub stocks: StockMarket,
    pub last_traded_price: HashMap<u64, f64>,
    pub cumulative_volume: HashMap<u64, u64>,
}

// This is the "pointer" to the shadow book that agents will hold.
pub type ShadowBookHandle = Arc<RwLock<MarketState>>;

impl MarketState {
    pub fn book(&self, stock_id: u64) -> Option<&OrderBook> {
        self.order_books.get(&stock_id)
    }
    pub fn get_mid_price(&self, stock_id: u64) -> Option<u64> {
        let book = self.book(stock_id)?;
        let best_bid = book.bids.keys().next_back()?;
        let best_ask = book.asks.keys().next()?;
        Some((best_bid + best_ask) / 2)
    }
}

/// A dedicated struct to hold all the outbound channels FROM the market TO a single agent.
pub struct AgentResponseChannels {
    pub ack_tx: Sender<Order>,
    pub trade_tx: Sender<Trade>,
}

pub struct Orchestra {
    // The Orchestra now holds the actors it will manage.
    // They are boxed to allow for different agent types (trait objects).
    agents: Vec<Box<dyn Agent>>,
    market: Market,
    shadow_handle: ShadowBookHandle,
}

impl Orchestra {
    pub fn new(
        agent_types: Vec<AgentType>,
        normal_processing: usize,
        premium_processing: usize,
    ) -> Self {
        println!("[Orchestra] Initializing simulation...");

        // === 1. Create Infrastructure ===
        let stock_market = StockMarket::new();
        // The shadow book is created empty. The Market is responsible for its initial state.
        let normal_shadow_book: ShadowBookHandle = Arc::new(RwLock::new(MarketState {
            order_books: HashMap::new(),
            stocks: stock_market.clone(), // Initial clone, market will overwrite
            last_traded_price: HashMap::new(),
            cumulative_volume: HashMap::new(),
        }));
        // For now, premium is just a clone of the normal setup handle.
        let premium_shadow_book: ShadowBookHandle = Arc::new(RwLock::new(MarketState {
            order_books: HashMap::new(),
            stocks: stock_market.clone(), // Initial clone, market will overwrite
            last_traded_price: HashMap::new(),
            cumulative_volume: HashMap::new(),
        }));
        println!("[Orchestra] Shared shadow books created.");

        let (order_tx, order_rx) = unbounded::<OrderRequest>();
        println!("[Orchestra] Central market order channel created.");

        // === 2. Instantiate Agents and Register Their Channels ===
        let mut agents: Vec<Box<dyn Agent>> = Vec::new();
        let mut registration_data: HashMap<usize, AgentResponseChannels> = HashMap::new();

        println!("[Orchestra] Creating {} agents...", agent_types.len());
        for (id, agent_type) in agent_types.into_iter().enumerate() {
            let (tx_ack, rx_ack) = unbounded::<Order>();
            let (tx_trade, rx_trade) = unbounded::<Trade>();

            let response_channels = AgentResponseChannels {
                ack_tx: tx_ack,
                trade_tx: tx_trade,
            };
            registration_data.insert(id, response_channels);

            let view_handle = match agent_type {
                _ => normal_shadow_book.clone(),
            };

            // Create the agent. Note we are only handling DumbMarket for now.
            let new_agent: Box<dyn Agent> = match agent_type {
                AgentType::DumbMarket => Box::new(DumbAgent::new(
                    id,
                    order_tx.clone(),
                    rx_ack,
                    rx_trade,
                    view_handle,
                )),
                AgentType::DumbLimit => Box::new(DumbLimitAgent::new(
                    id,
                    order_tx.clone(),
                    rx_ack,
                    rx_trade,
                    view_handle,
                )),
                AgentType::MarketMaker => Box::new(MarketMakerAgent::new(
                    id,
                    order_tx.clone(),
                    rx_ack,
                    rx_trade,
                    premium_shadow_book.clone(), // Use premium book for MarketMaker
                )),
                AgentType::IPO => Box::new(IpoAgent::new(
                    id,
                    order_tx.clone(),
                    rx_ack,
                    rx_trade,
                    view_handle,
                )),
                AgentType::WhaleAgent => Box::new(WhaleAgent::new(
                    id,
                    order_tx.clone(),
                    rx_ack,
                    rx_trade,
                    view_handle,
                )),
            };
            agents.push(new_agent);
        }
        println!("[Orchestra] {} agents instantiated.", agents.len());

        // === 3. Instantiate Market ===
        // The Market is given the receiving end of the order channel, the agent address book,
        // and a handle to the shadow book it must maintain.
        let market = Market::new(
            &stock_market,
            order_rx,
            registration_data,
            normal_shadow_book.clone(), // Pass the handle for the normal book
            normal_processing,
            premium_shadow_book.clone(), // Pass the handle for the premium book
            premium_processing,
        );
        println!("[Orchestra] Market instantiated and initialized.");

        // === 4. Return the fully prepared, but not yet running, Orchestra ===
        Orchestra {
            agents,
            market,
            shadow_handle: normal_shadow_book.clone(),
        }
    }

    pub fn get_shadow_handle(&self) -> ShadowBookHandle {
        self.shadow_handle.clone()
    }

    /// This method consumes the Orchestra and launches all actors in their own threads.
    /// It blocks until all simulation threads have completed.
    pub fn run(self) {
        println!("[Orchestra] Launching all actors...");

        let mut handles: Vec<JoinHandle<()>> = vec![];

        // First, move the market into its own thread.
        // We must use a mutable variable to move out of the struct.
        let mut market = self.market;
        let market_handle = thread::spawn(move || {
            market.run();
        });
        handles.push(market_handle);
        println!("[Orchestra] Market thread launched.");

        // Next, move each agent into its own thread.
        for mut agent in self.agents {
            let agent_handle = thread::spawn(move || {
                agent.run();
            });
            handles.push(agent_handle);
        }
        println!("[Orchestra] {} agent threads launched.", handles.len() - 1);

        // --- Wait for all threads to complete ---
        println!("[Orchestra] All actors running. Waiting for completion...");
        for handle in handles {
            handle.join().unwrap();
        }
        println!("[Orchestra] All threads have completed. Simulation finished.");
    }
}
