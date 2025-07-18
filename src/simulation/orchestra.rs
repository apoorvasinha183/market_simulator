// src/simulation/orchestra.rs

use crossbeam_channel::{Receiver, Sender, unbounded};
use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::agents::agent_trait::Agent;
use crate::agents::agent_type::AgentType;
use crate::agents::astrologer_agent::AstrologerAgent;
use crate::agents::customer_agent::CustomerAgent;
use crate::agents::dumb_agent::DumbAgent;
use crate::agents::dumb_limit_agent::DumbLimitAgent;
use crate::agents::ipo_agent::IpoAgent;
use crate::agents::market_maker_agent::MarketMakerAgent;
use crate::agents::momentum_agent::MomentumAgent;
use crate::agents::thermo_agent::ThermoAgent;
use crate::agents::web_proxy_agent::{ProxyRequest, WebProxyAgent};
use crate::agents::web_server::WebServerRunner;
use crate::agents::whale_agent::WhaleAgent;
use crate::default_stock_universe;
use crate::events::MarketEvent;
use crate::market::Market;
use crate::sentiment_engine::SentimentEngine;
use crate::simulation::candle_analyzer::{CandleAnalyzer, CandleDataHandle};
use crate::simulators::market_trait::Marketable;
use crate::simulators::order_book::OrderBook;
use crate::stocks::StockMarket;
use crate::types::order::{Order, OrderRequest, Trade};

// ----------------------------------------------------------------------------
//  Shadow Book Infrastructure
// ----------------------------------------------------------------------------

/// Events sent from the Market to the ShadowWorkers to update the view.
#[derive(Debug, Clone)]
pub enum ShadowEvent {
    LimitOrder(Order),
    MarketOrder(Order),
    CancelOrder { order_id: u64, agent_id: usize },
}

/// The internal, concurrent state used for building the back-buffer.
#[derive(Debug)]
pub struct ConcurrentMarketState {
    pub order_books: DashMap<u64, OrderBook>,
    pub stocks: StockMarket,
    pub last_traded_price: DashMap<u64, f64>,
    pub cumulative_volume: DashMap<u64, u64>,
}

/// The shared, read-only state that agents see. This uses standard HashMaps
/// for maximum read performance, as it's only ever written to in a single swap.
#[derive(Debug, Clone)]
pub struct MarketState {
    pub order_books: HashMap<u64, Arc<OrderBook>>,
    pub stocks: StockMarket,
    pub last_traded_price: HashMap<u64, f64>,
    pub cumulative_volume: HashMap<u64, u64>,
}

pub type ShadowBookHandle = Arc<RwLock<MarketState>>;

impl serde::Serialize for MarketState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("MarketState", 4)?;
        
        // Convert Arc<OrderBook> to OrderBook for serialization
        let order_books_for_serialization: HashMap<u64, &OrderBook> = self
            .order_books
            .iter()
            .map(|(k, v)| (*k, v.as_ref()))
            .collect();
            
        state.serialize_field("order_books", &order_books_for_serialization)?;
        state.serialize_field("stocks", &self.stocks)?;
        state.serialize_field("last_traded_price", &self.last_traded_price)?;
        state.serialize_field("cumulative_volume", &self.cumulative_volume)?;
        state.end()
    }
}

impl MarketState {
    /// Creates a new read-only MarketState from the concurrent back-buffer.
    /// This is the snapshot that gets swapped into the front-buffer for agents.
    pub fn from_concurrent(concurrent_state: &ConcurrentMarketState) -> Self {
        MarketState {
            order_books: concurrent_state
                .order_books
                .iter()
                .map(|entry| (*entry.key(), Arc::new(entry.value().clone())))
                .collect(),
            stocks: concurrent_state.stocks.clone(),
            last_traded_price: concurrent_state
                .last_traded_price
                .iter()
                .map(|entry| (*entry.key(), *entry.value()))
                .collect(),
            cumulative_volume: concurrent_state
                .cumulative_volume
                .iter()
                .map(|entry| (*entry.key(), *entry.value()))
                .collect(),
        }
    }

    pub fn book(&self, stock_id: u64) -> Option<Arc<OrderBook>> {
        self.order_books.get(&stock_id).cloned()
    }

    pub fn get_mid_price(&self, stock_id: u64) -> Option<u64> {
        let book = self.book(stock_id)?;
        let best_bid = book.bids.keys().next_back()?;
        let best_ask = book.asks.keys().next()?;
        Some((best_bid + best_ask) / 2)
    }

    pub fn get_spread(&self, stock_id: u64) -> Option<u64> {
        let book = self.book(stock_id)?;
        let best_bid = book.bids.keys().last()?;
        let best_ask = book.asks.keys().next()?;
        if *best_ask > *best_bid {
            Some(best_ask - best_bid)
        } else {
            None // Or handle crossed market case
        }
    }
}

/// Manages the parallel construction of the back-buffer and the final swap.
pub struct ShadowCoordinator {
    handle: ShadowBookHandle,
    update_receivers: HashMap<u64, Receiver<ShadowEvent>>,
    update_interval_ms: u64,
}

impl ShadowCoordinator {
    pub fn new(
        handle: ShadowBookHandle,
        update_receivers: HashMap<u64, Receiver<ShadowEvent>>,
        update_interval_ms: u64,
    ) -> Self {
        Self {
            handle,
            update_receivers,
            update_interval_ms,
        }
    }

    pub fn run(self) {
        let initial_state = self.handle.read().unwrap();
        let back_buffer = Arc::new(ConcurrentMarketState {
            order_books: initial_state
                .order_books
                .iter()
                .map(|(k, v)| (*k, (**v).clone()))
                .collect(),
            stocks: initial_state.stocks.clone(),
            last_traded_price: initial_state
                .last_traded_price
                .iter()
                .map(|(k, v)| (*k, *v))
                .collect(),
            cumulative_volume: initial_state
                .cumulative_volume
                .iter()
                .map(|(k, v)| (*k, *v))
                .collect(),
        });

        for (stock_id, rx) in self.update_receivers {
            let buffer_clone = back_buffer.clone();
            thread::spawn(move || {
                Self::run_builder_thread(rx, buffer_clone, stock_id);
            });
        }

        let handle_clone = self.handle.clone();
        thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_millis(self.update_interval_ms));
                let new_front_buffer = MarketState::from_concurrent(&back_buffer);
                let mut write_lock = handle_clone.write().unwrap();
                *write_lock = new_front_buffer;
            }
        });
    }

    fn run_builder_thread(
        rx: Receiver<ShadowEvent>,
        buffer: Arc<ConcurrentMarketState>,
        stock_id: u64,
    ) {
        while let Ok(event) = rx.recv() {
            let trades = match event {
                ShadowEvent::LimitOrder(mut order) => buffer
                    .order_books
                    .entry(stock_id)
                    .or_default()
                    .process_limit_order(&mut order),
                ShadowEvent::MarketOrder(order) => buffer
                    .order_books
                    .entry(stock_id)
                    .or_default()
                    .process_market_order(order.id, order.agent_id, order.side, order.volume),
                ShadowEvent::CancelOrder { order_id, agent_id } => {
                    if let Some(mut book) = buffer.order_books.get_mut(&stock_id) {
                        book.cancel_order(order_id, agent_id);
                    }
                    Vec::new()
                }
            };

            if !trades.is_empty() {
                let last_price = trades.last().unwrap().price as f64 / 100.0;
                buffer.last_traded_price.insert(stock_id, last_price);
                let total_volume: u64 = trades.iter().map(|t| t.volume).sum();
                buffer
                    .cumulative_volume
                    .entry(stock_id)
                    .and_modify(|v| *v += total_volume)
                    .or_insert(total_volume);
            }
        }
    }
}

// ----------------------------------------------------------------------------
//  Orchestra and Agent Setup
// ----------------------------------------------------------------------------

pub struct AgentResponseChannels {
    pub ack_tx: Sender<Order>,
    pub trade_tx: Sender<Trade>,
}

pub struct Orchestra {
    agents: Vec<Box<dyn Agent>>,
    pub market: Market,
    shadow_handle: ShadowBookHandle,
    candle_analyzer: CandleAnalyzer,
    candle_data_handle: CandleDataHandle,
    event_sender: Sender<MarketEvent>,
}

impl Orchestra {
    pub fn new(
        agent_types: Vec<AgentType>,
        normal_processing_ms: u64,
        premium_processing_ms: u64,
    ) -> Self {
        println!("[Orchestra] Initializing simulation...");

        let stock_market = StockMarket::from_universe(default_stock_universe());
        let (order_tx, order_rx) = unbounded::<OrderRequest>();
        let (event_tx, event_rx) = unbounded::<MarketEvent>();

        let mut normal_shadow_senders = HashMap::new();
        let mut normal_shadow_receivers = HashMap::new();
        let mut premium_shadow_senders = HashMap::new();
        let mut premium_shadow_receivers = HashMap::new();

        for stock in stock_market.get_all_stocks() {
            let (tx, rx) = unbounded();
            normal_shadow_senders.insert(stock.id, tx);
            normal_shadow_receivers.insert(stock.id, rx);
            let (tx_vip, rx_vip) = unbounded();
            premium_shadow_senders.insert(stock.id, tx_vip);
            premium_shadow_receivers.insert(stock.id, rx_vip);
        }

        let initial_state = || {
            let mut last_traded_price = HashMap::new();
            let mut cumulative_volume = HashMap::new();
            let mut order_books = HashMap::new();
            for s in stock_market.get_all_stocks() {
                order_books.insert(s.id, Arc::new(OrderBook::new()));
                last_traded_price.insert(s.id, s.initial_price);
                cumulative_volume.insert(s.id, 0);
            }
            MarketState {
                order_books,
                stocks: stock_market.clone(),
                last_traded_price,
                cumulative_volume,
            }
        };

        let normal_shadow_book: ShadowBookHandle = Arc::new(RwLock::new(initial_state()));
        let (trade_to_candle_tx, trade_to_candle_rx) = unbounded::<Trade>();
        let (candle_analyzer, candle_data_handle) = CandleAnalyzer::new(trade_to_candle_rx);
        let premium_shadow_book: ShadowBookHandle = Arc::new(RwLock::new(initial_state()));

        ShadowCoordinator::new(
            normal_shadow_book.clone(),
            normal_shadow_receivers,
            normal_processing_ms,
        )
        .run();

        ShadowCoordinator::new(
            premium_shadow_book.clone(),
            premium_shadow_receivers,
            premium_processing_ms,
        )
        .run();

        println!("[Orchestra] Shadow Coordinators launched.");

        let (proxy_request_tx, proxy_request_rx) = unbounded::<ProxyRequest>();
        let mut agents: Vec<Box<dyn Agent>> = Vec::new();
        let mut registration_data: HashMap<usize, AgentResponseChannels> = HashMap::new();

        for (id, agent_type) in agent_types.into_iter().enumerate() {
            let (tx_ack, rx_ack) = unbounded();
            let (tx_trade, rx_trade) = unbounded();
            registration_data.insert(
                id,
                AgentResponseChannels {
                    ack_tx: tx_ack,
                    trade_tx: tx_trade,
                },
            );
            // big moneh gets the premium view
            let view_handle = match agent_type {
                AgentType::MarketMaker => premium_shadow_book.clone(),
                _ => normal_shadow_book.clone(),
            };

            let new_agent: Box<dyn Agent> = match agent_type {
                AgentType::Astrologer => Box::new(AstrologerAgent::new(
                    id,
                    order_tx.clone(),
                    rx_ack,
                    rx_trade,
                    view_handle,
                    candle_data_handle.clone(),
                )),
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
                    view_handle,
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
                AgentType::MomentumAgent => Box::new(MomentumAgent::new(
                    id,
                    order_tx.clone(),
                    rx_ack,
                    rx_trade,
                    view_handle,
                )),
                AgentType::CustomerAgent => Box::new(CustomerAgent::new(
                    id,
                    order_tx.clone(),
                    rx_ack,
                    rx_trade,
                    view_handle,
                )),
                AgentType::WebProxyAgent => Box::new(WebProxyAgent::new(
                    id,
                    order_tx.clone(),
                    rx_ack,
                    rx_trade,
                    proxy_request_rx.clone(),
                )),
                AgentType::Thermodynamic {
                    initial_temperature,
                    specific_heat,
                    initial_chemical_potential,
                } => {
                    let event_rx_clone = event_rx.clone();
                    Box::new(ThermoAgent::new(
                        id,
                        order_tx.clone(),
                        rx_ack,
                        rx_trade,
                        event_rx_clone,
                        view_handle,
                        stock_market.clone(),
                        initial_temperature,
                        specific_heat,
                        initial_chemical_potential,
                    ))
                }
            };
            agents.push(new_agent);
        }
        println!("[Orchestra] {} agents instantiated.", agents.len());

        let market = Market::new(
            &stock_market,
            order_rx,
            registration_data,
            normal_shadow_senders,
            premium_shadow_senders,
            event_tx.clone(),
            trade_to_candle_tx,
        );
        println!("[Orchestra] Market instantiated.");

        let view_handle_clone = normal_shadow_book.clone();
        let candle_handle_clone = candle_data_handle.clone();

        thread::spawn(move || {
            WebServerRunner::run(view_handle_clone, candle_handle_clone, proxy_request_tx);
        });

        Orchestra {
            agents,
            market,
            shadow_handle: normal_shadow_book,
            candle_analyzer,
            candle_data_handle,
            event_sender: event_tx,
        }
    }

    pub fn get_shadow_handle(&self) -> ShadowBookHandle {
        self.shadow_handle.clone()
    }

    pub fn get_last_traded_prices(&self) -> Arc<RwLock<HashMap<u64, f64>>> {
        self.market.last_traded_price.clone()
    }

    pub fn get_candle_data_handle(&self) -> CandleDataHandle {
        self.candle_data_handle.clone()
    }

    pub fn run(self) {
        println!("[Orchestra] Launching all actors...");
        let mut handles: Vec<JoinHandle<()>> = vec![];

        let event_sender = self.event_sender.clone();
        handles.push(thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_millis(100));
                if event_sender.send(MarketEvent::Heartbeat).is_err() {
                    break;
                }
            }
        }));

        let stock_market = self.market.get_stock_market_clone();
        let sentiment_sender = self.event_sender.clone();
        handles.push(thread::spawn(move || {
            SentimentEngine::run(&stock_market, sentiment_sender);
        }));

        let candle_analyzer = self.candle_analyzer;
        handles.push(thread::spawn(move || candle_analyzer.run()));

        let mut market = self.market;
        handles.push(thread::spawn(move || market.run()));

        for mut agent in self.agents {
            handles.push(thread::spawn(move || agent.run()));
        }
        println!("[Orchestra] All actors launched.");
    }
}
