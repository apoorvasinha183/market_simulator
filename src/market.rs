// src/market.rs
//
// Multi-ticker version — each Symbol gets its own OrderBook.
// Agents already send a `symbol` inside every OrderRequest, so all
// routing happens here without touching the order-book logic.

use crate::{
    stocks::definitions::{default_stock_universe, Symbol},
    types::{Order, OrderRequest, Side, Trade},
    Agent, AgentType, DumbAgent, DumbLimitAgent, IpoAgent, MarketMakerAgent, MarketView,
    Marketable, OrderBook, WhaleAgent,
};

use std::any::Any;
use std::collections::HashMap;

// -----------------------------------------------------------------------------
//  Market
// -----------------------------------------------------------------------------
pub struct Market {
    /// One order-book per listed symbol.
    order_books: HashMap<Symbol, OrderBook>,

    /// All participating agents by ID.
    agents: HashMap<usize, Box<dyn Agent>>,

    /// Last traded price per symbol  (cents ⇒ divide by 100.0 for dollars).
    last_traded_price: HashMap<Symbol, f64>,

    initial_agent_types: Vec<AgentType>,
    order_id_counter: u64,

    /// Cumulative executed volume per symbol.
    cumulative_volume: HashMap<Symbol, u64>,
}

impl Market {
    // ---------------------------------------------------------------------
    //  Construction
    // ---------------------------------------------------------------------
    pub fn new(participant_types: &[AgentType]) -> Self {
        // --- 1. build the stock universe ---
        let mut order_books = HashMap::new();
        let mut last_traded_price = HashMap::new();
        let mut cumulative_volume = HashMap::new();

        for stock in default_stock_universe() {
            order_books.insert(stock.ticker.clone(), OrderBook::new());
            last_traded_price.insert(stock.ticker.clone(), stock.initial_price);
            cumulative_volume.insert(stock.ticker.clone(), 0);
        }

        // --- 2. instantiate agents ---
        let agents = participant_types
            .iter()
            .enumerate()
            .map(|(id, t)| (id, Self::make_agent(*t, id)))
            .collect();

        Self {
            order_books,
            agents,
            last_traded_price,
            initial_agent_types: participant_types.to_vec(),
            order_id_counter: 0,
            cumulative_volume,
        }
    }

    fn make_agent(t: AgentType, id: usize) -> Box<dyn Agent> {
        match t {
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

    // ---------------------------------------------------------------------
    //  Convenience getters
    // ---------------------------------------------------------------------
    pub fn get_order_book(&self, symbol: &Symbol) -> Option<&OrderBook> {
        self.order_books.get(symbol)
    }

    pub fn get_cumulative_volume(&self, symbol: &Symbol) -> Option<u64> {
        self.cumulative_volume.get(symbol).copied()
    }

    pub fn get_total_inventory(&self) -> i64 {
        self.agents.values().map(|a| a.get_inventory()).sum()
    }
}

// -----------------------------------------------------------------------------
//  Marketable
// -----------------------------------------------------------------------------
impl Marketable for Market {
    fn step(&mut self) -> f64 {
        // -------- Phase 1: Agents decide --------
        // Provide *all* books so complex agents can inspect cross-asset state.
        let market_view = MarketView {
            order_books: &self.order_books,
        };

        let mut requests = Vec::<OrderRequest>::new();
        let mut ids: Vec<_> = self.agents.keys().cloned().collect();
        ids.sort_unstable();

        for id in &ids {
            if let Some(a) = self.agents.get_mut(id) {
                requests.extend(a.decide_actions(&market_view));
            }
        }

        // -------- Phase 2: Execute requests --------
        let mut trades_this_tick = Vec::<Trade>::new();

        for req in requests {
            match req {
                OrderRequest::LimitOrder {
                    agent_id,
                    symbol,
                    side,
                    price,
                    volume,
                } => {
                    let mut order = Order {
                        id: self.next_order_id(),
                        agent_id,
                        symbol: symbol.clone(),
                        side,
                        price,
                        volume,
                        filled: 0,
                    };
                    if let Some(a) = self.agents.get_mut(&agent_id) {
                        a.acknowledge_order(order);
                    }
                    if let Some(book) = self.order_books.get_mut(&symbol) {
                        trades_this_tick.extend(book.process_limit_order(&mut order));
                    }
                }

                OrderRequest::MarketOrder {
                    agent_id,
                    symbol,
                    side,
                    volume,
                } => {
                    // Use last traded price of that symbol as reference.
                    let px_cents = self
                        .last_traded_price
                        .get(&symbol)
                        .copied()
                        .unwrap_or(150.00) // fallback
                        * 100.0;
                    let order = Order {
                        id: self.next_order_id(),
                        agent_id,
                        symbol: symbol.clone(),
                        side,
                        price: px_cents.round() as u64,
                        volume,
                        filled: 0,
                    };
                    if let Some(a) = self.agents.get_mut(&agent_id) {
                        a.acknowledge_order(order);
                    }
                    if let Some(book) = self.order_books.get_mut(&symbol) {
                        trades_this_tick.extend(book.process_market_order(agent_id, side, volume));
                    }
                }

                OrderRequest::CancelOrder {
                    agent_id,
                    order_id,
                } => {
                    // We don’t know which book holds the order; try both sides quickly.
                    for book in self.order_books.values_mut() {
                        if book.cancel_order(order_id, agent_id) {
                            break;
                        }
                    }
                }
            }
        }

        // -------- Phase 3: Margin calls --------
        let mut margin_reqs = Vec::<OrderRequest>::new();
        for id in &ids {
            if let Some(a) = self.agents.get_mut(id) {
                margin_reqs.extend(a.margin_call());
            }
        }
        for req in margin_reqs {
            if let OrderRequest::MarketOrder {
                agent_id,
                symbol,
                side,
                volume,
            } = req
            {
                if let Some(book) = self.order_books.get_mut(&symbol) {
                    trades_this_tick.extend(book.process_market_order(agent_id, side, volume));
                }
            }
        }

        // -------- Phase 4: Update portfolios --------
        for tr in &trades_this_tick {
            if let Some(taker) = self.agents.get_mut(&tr.taker_agent_id) {
                let delta = if tr.taker_side == Side::Buy {
                    tr.volume as i64
                } else {
                    -(tr.volume as i64)
                };
                taker.update_portfolio(delta, tr);
            }
            if let Some(maker) = self.agents.get_mut(&tr.maker_agent_id) {
                let delta = if tr.taker_side == Side::Sell {
                    tr.volume as i64
                } else {
                    -(tr.volume as i64)
                };
                maker.update_portfolio(delta, tr);
            }
        }

        // -------- Phase 5: Market-level bookkeeping --------
        if let Some(last) = trades_this_tick.last() {
            self.last_traded_price
                .insert(last.symbol.clone(), last.price as f64 / 100.0);
        }
        for tr in &trades_this_tick {
            *self.cumulative_volume.entry(tr.symbol.clone()).or_insert(0) += tr.volume;
        }

        // Return *any* one price (first symbol) so existing callers still work.
        self.last_traded_price
            .values()
            .next()
            .copied()
            .unwrap_or(150.00)
    }

    fn current_price(&self) -> f64 {
        self.last_traded_price
            .values()
            .next()
            .copied()
            .unwrap_or(150.00)
    }

    fn reset(&mut self) {
        // Re-spin agents
        self.agents = self
            .initial_agent_types
            .iter()
            .enumerate()
            .map(|(id, t)| (id, Self::make_agent(*t, id)))
            .collect();

        // Reset books and counters
        for book in self.order_books.values_mut() {
            *book = OrderBook::new();
        }
        for price in self.last_traded_price.values_mut() {
            *price = 150.00;
        }
        for vol in self.cumulative_volume.values_mut() {
            *vol = 0;
        }
        self.order_id_counter = 0;
    }

    /// Return the full map so GUI/tests can pick any symbol.
    fn get_order_book(&self) -> Option<&OrderBook> {
        // default to the first book for backward compatibility
        self.order_books.values().next()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
