// src/market.rs
//
// Multi-ticker engine: one OrderBook per stock-id.
// All routing keys on `stock_id: u64`; no String clones on the hot path.

use std::{any::Any, collections::HashMap};

use crate::{
    Agent, AgentType, DumbAgent, DumbLimitAgent, IpoAgent, MarketMakerAgent, MarketView,
    Marketable, OrderBook, WhaleAgent,
    stocks::definitions::StockMarket,
    types::{Order, OrderRequest, Side, Trade},
};

// -----------------------------------------------------------------------------
//  Market
// -----------------------------------------------------------------------------
pub struct Market {
    /* static universe */
    stocks: StockMarket,

    /* per-symbol state */
    order_books: HashMap<u64, OrderBook>, // id → book
    last_traded_price: HashMap<u64, f64>, // id → dollars
    cumulative_volume: HashMap<u64, u64>, // id → shares

    /* participants */
    agents: HashMap<usize, Box<dyn Agent>>,
    initial_agent_types: Vec<AgentType>,

    /* counters */
    order_id_counter: u64,
}

impl Market {
    // ---------------------------------------------------------------------
    //  Construction
    // ---------------------------------------------------------------------
    pub fn new(participant_types: &[AgentType], stocks: StockMarket) -> Self {
        /* build empty books + price/vol maps */
        let mut order_books = HashMap::new();
        let mut last_traded_price = HashMap::new();
        let mut cumulative_volume = HashMap::new();

        for s in stocks.get_all_stocks() {
            order_books.insert(s.id, OrderBook::new());
            last_traded_price.insert(s.id, s.initial_price);
            cumulative_volume.insert(s.id, 0);
        }

        /* instantiate agents */
        let agents = participant_types
            .iter()
            .enumerate()
            .map(|(id, t)| (id, Self::spawn_agent(*t, id)))
            .collect();

        Self {
            stocks,
            order_books,
            last_traded_price,
            cumulative_volume,
            agents,
            initial_agent_types: participant_types.to_vec(),
            order_id_counter: 0,
        }
    }

    fn spawn_agent(t: AgentType, id: usize) -> Box<dyn Agent> {
        match t {
            AgentType::DumbMarket => Box::new(DumbAgent::new(id)),
            AgentType::DumbLimit => Box::new(DumbLimitAgent::new(id)),
            AgentType::MarketMaker => Box::new(MarketMakerAgent::new(id)),
            AgentType::IPO => Box::new(IpoAgent::new(id)),
            AgentType::WhaleAgent => Box::new(WhaleAgent::new(id)),
        }
    }

    #[inline]
    fn next_order_id(&mut self) -> u64 {
        self.order_id_counter += 1;
        self.order_id_counter
    }

    // ---------------------------------------------------------------------
    //  Convenience getters
    // ---------------------------------------------------------------------
    pub fn order_book(&self, stock_id: u64) -> Option<&OrderBook> {
        self.order_books.get(&stock_id)
    }

    pub fn cumulative_volume(&self, stock_id: u64) -> Option<u64> {
        self.cumulative_volume.get(&stock_id).copied()
    }

    pub fn total_inventory(&self) -> i64 {
        self.agents.values().map(|a| a.get_inventory()).sum()
    }
    #[inline]
    pub fn order_books(&self) -> &HashMap<u64, OrderBook> {
        &self.order_books
    }

    #[inline]
    pub fn last_price(&self, stock_id: u64) -> f64 {
        *self.last_traded_price.get(&stock_id).unwrap_or(&150.0)
    }

    #[inline]
    pub fn last_price_map_iter(&self) -> impl Iterator<Item = (&u64, &f64)> {
        self.last_traded_price.iter()
    }
    pub fn ticker(&self, stock_id: u64) -> &str {
        self.stocks
            .get_ticker_by_id(stock_id)
            .map(String::as_str)
            .unwrap_or("??")
    }
}

// -----------------------------------------------------------------------------
//  Marketable
// -----------------------------------------------------------------------------
impl Marketable for Market {
    fn step(&mut self) -> f64 {
        /* -------- Phase 1: agent decisions -------- */
        let view = MarketView {
            order_books: &self.order_books,
            stocks: &self.stocks,
        };

        let mut requests = Vec::<OrderRequest>::new();
        let mut ids: Vec<_> = self.agents.keys().copied().collect();
        ids.sort_unstable();

        for id in &ids {
            if let Some(a) = self.agents.get_mut(id) {
                requests.extend(a.decide_actions(&view));
            }
        }

        /* -------- Phase 2: execute orders -------- */
        let mut trades = Vec::<Trade>::new();

        for req in requests {
            match req {
                OrderRequest::LimitOrder {
                    agent_id,
                    stock_id,
                    side,
                    price,
                    volume,
                } => {
                    let mut o = Order {
                        id: self.next_order_id(),
                        agent_id,
                        stock_id,
                        side,
                        price,
                        volume,
                        filled: 0,
                    };
                    self.agents.get_mut(&agent_id).unwrap().acknowledge_order(o);
                    if let Some(book) = self.order_books.get_mut(&stock_id) {
                        trades.extend(book.process_limit_order(&mut o));
                    }
                }
                OrderRequest::MarketOrder {
                    agent_id,
                    stock_id,
                    side,
                    volume,
                } => {
                    let px_cents = (self
                        .last_traded_price
                        .get(&stock_id)
                        .copied()
                        .unwrap_or(150.0)
                        * 100.0)
                        .round() as u64;
                    let o = Order {
                        id: self.next_order_id(),
                        agent_id,
                        stock_id,
                        side,
                        price: px_cents,
                        volume,
                        filled: 0,
                    };
                    self.agents.get_mut(&agent_id).unwrap().acknowledge_order(o);
                    if let Some(book) = self.order_books.get_mut(&stock_id) {
                        trades.extend(book.process_market_order(agent_id, side, volume));
                    }
                }
                OrderRequest::CancelOrder { agent_id, order_id } => {
                    for book in self.order_books.values_mut() {
                        if book.cancel_order(order_id, agent_id) {
                            break;
                        }
                    }
                }
            }
        }

        /* -------- Phase 3: margin calls -------- */
        let mut margin = Vec::<OrderRequest>::new();
        for id in &ids {
            if let Some(a) = self.agents.get_mut(id) {
                margin.extend(a.margin_call());
            }
        }
        for req in margin {
            if let OrderRequest::MarketOrder {
                agent_id,
                stock_id,
                side,
                volume,
            } = req
            {
                if let Some(book) = self.order_books.get_mut(&stock_id) {
                    trades.extend(book.process_market_order(agent_id, side, volume));
                }
            }
        }

        /* -------- Phase 4: update portfolios -------- */
        for tr in &trades {
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

        /* -------- Phase 5: book-level bookkeeping -------- */
        if let Some(last) = trades.last() {
            self.last_traded_price
                .insert(last.stock_id, last.price as f64 / 100.0);
        }
        for tr in &trades {
            *self.cumulative_volume.entry(tr.stock_id).or_insert(0) += tr.volume;
        }

        /* Return any price (first) for backward compatibility */
        self.last_traded_price
            .values()
            .next()
            .copied()
            .unwrap_or(150.0)
    }

    fn current_price(&self) -> f64 {
        self.last_traded_price
            .values()
            .next()
            .copied()
            .unwrap_or(150.0)
    }

    fn reset(&mut self) {
        /* agents */
        self.agents = self
            .initial_agent_types
            .iter()
            .enumerate()
            .map(|(id, t)| (id, Self::spawn_agent(*t, id)))
            .collect();

        /* per-symbol state */
        // fresh books
        for book in self.order_books.values_mut() {
            *book = OrderBook::new();
        }

        // restore initial prices **per instrument** (instead of hard-coding 150)
        for s in self.stocks.get_all_stocks() {
            self.last_traded_price.insert(s.id, s.initial_price);
            self.cumulative_volume.insert(s.id, 0);
        }

        self.order_id_counter = 0;
    }

    fn get_order_book(&self) -> Option<&OrderBook> {
        self.order_books.values().next() // for legacy callers
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
    fn run(&mut self) {
        loop {
            self.step();
        }
    }
}
