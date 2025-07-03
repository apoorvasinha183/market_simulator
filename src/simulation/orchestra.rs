// src/orchestra.rs

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

// --- Crate-level imports ---
use crate::agents::agent_trait::Agent;
use crate::agents::agent_type::AgentType;
use crate::agents::customer_agent::CustomerAgent;
use crate::agents::dumb_agent::DumbAgent;
use crate::agents::dumb_limit_agent::DumbLimitAgent;
use crate::agents::ipo_agent::IpoAgent;
use crate::agents::market_maker_agent::MarketMakerAgent;
use crate::agents::thermo_agent::ThermoAgent;
use crate::agents::whale_agent::WhaleAgent;
use crate::events::MarketEvent;
use crate::market::Market;
use crate::sentiment_engine::SentimentEngine;
use crate::simulators::market_trait::Marketable;
use crate::simulators::order_book::OrderBook;
use crate::stocks::StockMarket;
use crate::types::order::{Order, OrderRequest, Trade};
use crossbeam_channel::{Receiver, Sender, unbounded};

// --- This is the shared state agents will read from. ---
#[derive(Debug, Clone)]
pub struct MarketState {
    pub order_books: HashMap<u64, OrderBook>,
    pub stocks: StockMarket,
    pub last_traded_price: HashMap<u64, f64>,
    pub cumulative_volume: HashMap<u64, u64>,
}

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

pub struct AgentResponseChannels {
    pub ack_tx: Sender<Order>,
    pub trade_tx: Sender<Trade>,
}

pub struct Orchestra {
    agents: Vec<Box<dyn Agent>>,
    market: Market,
    shadow_handle: ShadowBookHandle,
    // Keep the sender to spawn the heartbeat thread
    event_sender: Sender<MarketEvent>,
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

        let normal_shadow_book: ShadowBookHandle = Arc::new(RwLock::new(MarketState {
            order_books: HashMap::new(),
            stocks: stock_market.clone(),
            last_traded_price: HashMap::new(),
            cumulative_volume: HashMap::new(),
        }));
        let premium_shadow_book: ShadowBookHandle = Arc::new(RwLock::new(MarketState {
            order_books: HashMap::new(),
            stocks: stock_market.clone(),
            last_traded_price: HashMap::new(),
            cumulative_volume: HashMap::new(),
        }));
        println!("[Orchestra] Shared shadow books created.");

        let (order_tx, order_rx) = unbounded::<OrderRequest>();
        println!("[Orchestra] Central market order channel created.");

        // === NEW: Create the central event bus ===
        let (event_tx, event_rx) = unbounded::<MarketEvent>();
        println!("[Orchestra] Central event bus created.");

        // === 2. Instantiate Agents and Register Their Channels ===
        let mut agents: Vec<Box<dyn Agent>> = Vec::new();
        let mut registration_data: HashMap<usize, AgentResponseChannels> = HashMap::new();

        println!("[Orchestra] Creating {} agents...", agent_types.len());
        for (id, agent_type) in agent_types.into_iter().enumerate() {
            let (tx_ack, rx_ack) = unbounded::<Order>();
            let (tx_trade, rx_trade) = unbounded::<Trade>();

            registration_data.insert(id, AgentResponseChannels { ack_tx: tx_ack, trade_tx: tx_trade });

            let view_handle = match agent_type {
                AgentType::MarketMaker => premium_shadow_book.clone(),
                _ => normal_shadow_book.clone(),
            };

            let new_agent: Box<dyn Agent> = match agent_type {
                AgentType::DumbMarket => {
                    let event_rx_clone = event_rx.clone();
                    Box::new(ThermoAgent::new(id, order_tx.clone(), rx_ack, rx_trade, event_rx_clone, view_handle, 0.1)) // Low specific heat for meme traders
                }
                AgentType::DumbLimit => {
                    let event_rx_clone = event_rx.clone();
                    Box::new(ThermoAgent::new(id, order_tx.clone(), rx_ack, rx_trade, event_rx_clone, view_handle, 1.0)) // High specific heat for value traders
                }
                AgentType::MarketMaker => Box::new(MarketMakerAgent::new(id, order_tx.clone(), rx_ack, rx_trade, view_handle)),
                AgentType::IPO => Box::new(IpoAgent::new(id, order_tx.clone(), rx_ack, rx_trade, view_handle)),
                AgentType::WhaleAgent => Box::new(WhaleAgent::new(id, order_tx.clone(), rx_ack, rx_trade, view_handle)),
                AgentType::CustomerAgent => Box::new(CustomerAgent::new(id, order_tx.clone(), rx_ack, rx_trade, view_handle)),
                AgentType::Thermodynamic => {
                    // Thermodynamic agents get a receiver for the event bus
                    let event_rx_clone = event_rx.clone();
                    Box::new(ThermoAgent::new(id, order_tx.clone(), rx_ack, rx_trade, event_rx_clone, view_handle, 0.5))
                }
            };
            agents.push(new_agent);
        }
        println!("[Orchestra] {} agents instantiated.", agents.len());

        // === 3. Instantiate Market ===
        let market = Market::new(
            &stock_market,
            order_rx,
            registration_data,
            normal_shadow_book.clone(),
            normal_processing,
            premium_shadow_book.clone(),
            premium_processing,
            event_tx.clone(), // Give the market a sender
        );
        println!("[Orchestra] Market instantiated and initialized.");

        // === 4. Return the fully prepared, but not yet running, Orchestra ===
        Orchestra {
            agents,
            market,
            shadow_handle: normal_shadow_book.clone(),
            event_sender: event_tx,
        }
    }

    pub fn get_shadow_handle(&self) -> ShadowBookHandle {
        self.shadow_handle.clone()
    }

    /// This method consumes the Orchestra and launches all actors in their own threads.
    pub fn run(self) {
        println!("[Orchestra] Launching all actors...");

        let mut handles: Vec<JoinHandle<()>> = vec![];

        // --- NEW: Launch the Heartbeat Thread ---
        let event_sender = self.event_sender.clone();
        let heartbeat_handle = thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_millis(100));
                if event_sender.send(MarketEvent::Heartbeat).is_err() {
                    // Main channel closed, exit
                    break;
                }
            }
        });
        handles.push(heartbeat_handle);
        println!("[Orchestra] Heartbeat thread launched.");

        // --- NEW: Launch the Sentiment Engine ---
        let stock_market = self.market.get_stock_market_clone(); // Need a way to get this
        let sentiment_sender = self.event_sender.clone();
        let sentiment_handle = thread::spawn(move || {
            SentimentEngine::run(&stock_market, sentiment_sender);
        });
        handles.push(sentiment_handle);
        println!("[Orchestra] SentimentEngine thread launched.");


        // First, move the market into its own thread.
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
        println!("[Orchestra] {} agent threads launched.", handles.len() - 2); // -2 for market and heartbeat

        // In a real app, you'd join these handles. For the sim, we let them run.
    }
}
