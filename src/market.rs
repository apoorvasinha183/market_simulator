// src/market.rs
use std::collections::HashMap;
use std::thread;

use crate::events::MarketEvent;
use crate::simulation::orchestra::{AgentResponseChannels, MarketState, ShadowBookHandle};
use crate::{
    Marketable, OrderBook,
    stocks::definitions::StockMarket,
    types::{Order, OrderRequest, Trade},
};
use crossbeam_channel::{Receiver, Sender, unbounded};

#[derive(Debug, Clone)]
enum ShadowEvent {
    LimitOrder(Order),
    MarketOrder(Order),
    CancelOrder { order_id: u64, agent_id: usize },
}

pub struct Market {
    order_books: HashMap<u64, OrderBook>,
    last_traded_price: HashMap<u64, f64>,
    cumulative_volume: HashMap<u64, u64>,
    order_id_counter: u64,
    stock_market: StockMarket,
    order_rx: Receiver<OrderRequest>,
    agent_channels: HashMap<usize, AgentResponseChannels>,
    shadow_update_tx: Sender<ShadowEvent>,
    vip_shadow_update_tx: Sender<ShadowEvent>,
    event_tx: Sender<MarketEvent>,
}

impl Market {
    pub fn new(
        stocks: &StockMarket,
        order_rx: Receiver<OrderRequest>,
        agent_channels: HashMap<usize, AgentResponseChannels>,
        shadow_book_handle: ShadowBookHandle,
        update_threshold: usize,
        vip_book_handle: ShadowBookHandle,
        vip_update_threshold: usize,
        event_tx: Sender<MarketEvent>,
    ) -> Self {
        let mut order_books = HashMap::new();
        let mut last_traded_price = HashMap::new();
        let mut cumulative_volume = HashMap::new();

        for s in stocks.get_all_stocks() {
            order_books.insert(s.id, OrderBook::new());
            last_traded_price.insert(s.id, s.initial_price);
            cumulative_volume.insert(s.id, 0);
        }

        {
            let mut state_lock = shadow_book_handle.write().unwrap();
            state_lock.stocks = stocks.clone();
            state_lock.order_books = order_books.clone();
            state_lock.last_traded_price = last_traded_price.clone();
            state_lock.cumulative_volume = cumulative_volume.clone();
        }
        {
            let mut state_lock = vip_book_handle.write().unwrap();
            state_lock.stocks = stocks.clone();
            state_lock.order_books = order_books.clone();
            state_lock.last_traded_price = last_traded_price.clone();
            state_lock.cumulative_volume = cumulative_volume.clone();
        }

        let (shadow_update_tx, shadow_update_rx) = unbounded::<ShadowEvent>();
        let (vip_shadow_update_tx, vip_shadow_update_rx) = unbounded::<ShadowEvent>();
        Self::spawn_shadow_worker(
            shadow_update_rx,
            shadow_book_handle.clone(),
            update_threshold,
        );
        Self::spawn_shadow_worker(vip_shadow_update_rx, vip_book_handle, vip_update_threshold);
        println!("[Market] Connected agents: {:?}", agent_channels.keys());
        Self {
            order_books,
            last_traded_price,
            cumulative_volume,
            order_id_counter: 0,
            stock_market: stocks.clone(),
            order_rx,
            agent_channels,
            shadow_update_tx,
            vip_shadow_update_tx,
            event_tx,
        }
    }

    pub fn get_stock_market_clone(&self) -> StockMarket {
        self.stock_market.clone()
    }

    fn spawn_shadow_worker(
        update_rx: Receiver<ShadowEvent>,
        shadow_book_handle: ShadowBookHandle,
        update_threshold: usize,
    ) {
        thread::spawn(move || {
            let mut back_buffer: MarketState = shadow_book_handle.read().unwrap().clone();
            let mut event_log: Vec<ShadowEvent> = Vec::with_capacity(update_threshold);
            let mut event_counter = 0;

            while let Ok(event) = update_rx.recv() {
                event_log.push(event.clone());
                let trades = match event {
                    ShadowEvent::LimitOrder(mut order) => {
                        if let Some(book) = back_buffer.order_books.get_mut(&order.stock_id) {
                            book.process_limit_order(&mut order)
                        } else {
                            Vec::new()
                        }
                    }
                    ShadowEvent::MarketOrder(order) => {
                        if let Some(book) = back_buffer.order_books.get_mut(&order.stock_id) {
                            book.process_market_order(order.agent_id, order.side, order.volume)
                        } else {
                            Vec::new()
                        }
                    }
                    ShadowEvent::CancelOrder { order_id, agent_id } => {
                        for book in back_buffer.order_books.values_mut() {
                            if book.cancel_order(order_id, agent_id) {
                                break;
                            }
                        }
                        Vec::new()
                    }
                };

                for trade in trades {
                    if let Some(price_mut) = back_buffer.last_traded_price.get_mut(&trade.stock_id)
                    {
                        *price_mut = trade.price as f64 / 100.0;
                    }
                    if let Some(vol_mut) = back_buffer.cumulative_volume.get_mut(&trade.stock_id) {
                        *vol_mut += trade.volume;
                    }
                }

                event_counter += 1;

                if event_counter >= update_threshold {
                    {
                        let mut state_lock = shadow_book_handle.write().unwrap();
                        std::mem::swap(&mut back_buffer, &mut *state_lock);
                    }

                    for logged_event in &event_log {
                        let trades = match logged_event {
                            ShadowEvent::LimitOrder(order) => {
                                if let Some(book) = back_buffer.order_books.get_mut(&order.stock_id)
                                {
                                    let mut o = *order;
                                    book.process_limit_order(&mut o)
                                } else {
                                    Vec::new()
                                }
                            }
                            ShadowEvent::MarketOrder(order) => {
                                if let Some(book) = back_buffer.order_books.get_mut(&order.stock_id)
                                {
                                    book.process_market_order(
                                        order.agent_id,
                                        order.side,
                                        order.volume,
                                    )
                                } else {
                                    Vec::new()
                                }
                            }
                            ShadowEvent::CancelOrder { order_id, agent_id } => {
                                for book in back_buffer.order_books.values_mut() {
                                    if book.cancel_order(*order_id, *agent_id) {
                                        break;
                                    }
                                }
                                Vec::new()
                            }
                        };

                        for trade in trades {
                            if let Some(price_mut) =
                                back_buffer.last_traded_price.get_mut(&trade.stock_id)
                            {
                                *price_mut = trade.price as f64 / 100.0;
                            }
                            if let Some(vol_mut) =
                                back_buffer.cumulative_volume.get_mut(&trade.stock_id)
                            {
                                *vol_mut += trade.volume;
                            }
                        }
                    }
                    event_log.clear();
                    event_counter = 0;
                }
            }
        });
    }

    #[inline]
    fn next_order_id(&mut self) -> u64 {
        self.order_id_counter += 1;
        self.order_id_counter
    }

    fn process_request(&mut self, req: OrderRequest) {
        let mut trades = Vec::<Trade>::new();

        match req {
            OrderRequest::LimitOrder {
                agent_id,
                stock_id,
                side,
                price,
                volume,
            } => {
                let mut order = Order {
                    id: self.next_order_id(),
                    agent_id,
                    stock_id,
                    side,
                    price,
                    volume,
                    filled: 0,
                };
                if let Some(ch) = self.agent_channels.get(&agent_id) {
                    ch.ack_tx.send(order).unwrap();
                }
                if let Some(book) = self.order_books.get_mut(&stock_id) {
                    trades.extend(book.process_limit_order(&mut order));
                }
                self.shadow_update_tx
                    .send(ShadowEvent::LimitOrder(order))
                    .unwrap();
                self.vip_shadow_update_tx
                    .send(ShadowEvent::LimitOrder(order))
                    .unwrap();
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
                let order = Order {
                    id: self.next_order_id(),
                    agent_id,
                    stock_id,
                    side,
                    volume,
                    price: px_cents,
                    filled: 0,
                };

                if let Some(ch) = self.agent_channels.get(&agent_id) {
                    ch.ack_tx.send(order).unwrap();
                }
                if let Some(book) = self.order_books.get_mut(&stock_id) {
                    trades.extend(book.process_market_order(agent_id, side, volume));
                }
                self.shadow_update_tx
                    .send(ShadowEvent::MarketOrder(order))
                    .unwrap();
                self.vip_shadow_update_tx
                    .send(ShadowEvent::MarketOrder(order))
                    .unwrap();
            }
            OrderRequest::CancelOrder { agent_id, order_id } => {
                for book in self.order_books.values_mut() {
                    if book.cancel_order(order_id, agent_id) {
                        break;
                    }
                }
                self.shadow_update_tx
                    .send(ShadowEvent::CancelOrder { order_id, agent_id })
                    .unwrap();
                self.vip_shadow_update_tx
                    .send(ShadowEvent::CancelOrder { order_id, agent_id })
                    .unwrap();
            }
        }

        for tr in &trades {
            self.event_tx
                .send(MarketEvent::TradeOccurred(*tr))
                .unwrap_or_else(|e| {
                    eprintln!("[Market] Failed to broadcast trade event: {}", e);
                });

            if let Some(taker_ch) = self.agent_channels.get(&tr.taker_agent_id) {
                taker_ch.trade_tx.send(*tr).unwrap();
            }
            if let Some(maker_ch) = self.agent_channels.get(&tr.maker_agent_id) {
                maker_ch.trade_tx.send(*tr).unwrap();
            }
        }

        if let Some(last) = trades.last() {
            self.last_traded_price
                .insert(last.stock_id, last.price as f64 / 100.0);
        }
        for tr in &trades {
            *self.cumulative_volume.entry(tr.stock_id).or_insert(0) += tr.volume;
        }
    }
}

impl Marketable for Market {
    fn run(&mut self) {
        while let Ok(req) = self.order_rx.recv() {
            self.process_request(req);
        }
    }
    fn step(&mut self) -> f64 {
        0.0
    }
    fn current_price(&self) -> f64 {
        0.0
    }
    fn reset(&mut self) {}
    fn get_order_book(&self) -> Option<&OrderBook> {
        None
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
