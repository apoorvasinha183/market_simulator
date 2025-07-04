// src/market.rs
use crate::events::MarketEvent;
use crate::simulation::orchestra::{AgentResponseChannels, MarketState, ShadowBookHandle};
use crate::simulators::async_order_book::AsyncOrderBook;
use crate::{
    Marketable, OrderBook,
    stocks::definitions::StockMarket,
    types::{Order, OrderRequest, Trade},
};
use crossbeam_channel::{Receiver, Sender, unbounded};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::thread;

#[derive(Debug, Clone)]
enum ShadowEvent {
    LimitOrder(Order),
    MarketOrder(Order),
    CancelOrder { order_id: u64, agent_id: usize },
}

pub struct Market {
    // Senders to the dedicated order book for each stock
    order_txs: HashMap<u64, Sender<OrderRequest>>,
    // Receiver for all incoming order requests from agents
    order_rx: Receiver<OrderRequest>,
    // Globally unique order ID counter
    order_id_counter: Arc<RwLock<u64>>,
    // Map from global order_id to stock_id for efficient cancellation routing
    order_id_to_stock_id_map: Arc<RwLock<HashMap<u64, u64>>>,
    // Channels to send acks and trades back to agents
    agent_channels: Arc<HashMap<usize, AgentResponseChannels>>,
    // Shadow book mechanism (unchanged)
    shadow_update_tx: Sender<ShadowEvent>,
    vip_shadow_update_tx: Sender<ShadowEvent>,
    // Central event bus for the simulation
    event_tx: Sender<MarketEvent>,
    // Shared state for last traded prices
    pub last_traded_price: Arc<RwLock<HashMap<u64, f64>>>,
    stock_market: StockMarket,
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
        let mut order_txs = HashMap::new();
        let (trade_tx, trade_rx) = unbounded::<Trade>();
        let mut last_traded_price_map = HashMap::new();

        // Initialize shadow books and last traded prices
        let mut initial_order_books = HashMap::new();
        let mut cumulative_volume = HashMap::new();
        for s in stocks.get_all_stocks() {
            initial_order_books.insert(s.id, OrderBook::new());
            last_traded_price_map.insert(s.id, s.initial_price);
            cumulative_volume.insert(s.id, 0);
        }
        let last_traded_price = Arc::new(RwLock::new(last_traded_price_map));

        // Spawn an async order book for each stock
        for stock in stocks.get_all_stocks() {
            let (order_tx, stock_trade_rx) = AsyncOrderBook::new();
            order_txs.insert(stock.id, order_tx);

            // Fan-in all trade channels into one
            let trade_tx_clone = trade_tx.clone();
            thread::spawn(move || {
                while let Ok(trade) = stock_trade_rx.recv() {
                    if trade_tx_clone.send(trade).is_err() {
                        break; // Main trade processor has shut down
                    }
                }
            });
        }
        // The single trade_tx is dropped here, which is fine as all clones are now in threads.

        // Setup shadow book state
        {
            let mut state_lock = shadow_book_handle.write().unwrap();
            state_lock.stocks = stocks.clone();
            state_lock.order_books = initial_order_books.clone();
            state_lock.last_traded_price = last_traded_price.read().unwrap().clone();
            state_lock.cumulative_volume = cumulative_volume.clone();
        }
        {
            let mut state_lock = vip_book_handle.write().unwrap();
            state_lock.stocks = stocks.clone();
            state_lock.order_books = initial_order_books;
            state_lock.last_traded_price = last_traded_price.read().unwrap().clone();
            state_lock.cumulative_volume = cumulative_volume;
        }

        // Spawn shadow workers (unchanged)
        let (shadow_update_tx, shadow_update_rx) = unbounded::<ShadowEvent>();
        let (vip_shadow_update_tx, vip_shadow_update_rx) = unbounded::<ShadowEvent>();
        Self::spawn_shadow_worker(shadow_update_rx, shadow_book_handle, update_threshold);
        Self::spawn_shadow_worker(vip_shadow_update_rx, vip_book_handle, vip_update_threshold);

        let agent_channels_arc = Arc::new(agent_channels);
        let order_id_to_stock_id_map = Arc::new(RwLock::new(HashMap::new()));

        // Spawn the single, dedicated trade processor thread
        Self::spawn_trade_processor(
            trade_rx.clone(),
            agent_channels_arc.clone(),
            event_tx.clone(),
            last_traded_price.clone(),
            order_id_to_stock_id_map.clone(),
        );

        println!("[Market] Connected agents: {:?}", agent_channels_arc.keys());

        Self {
            order_txs,
            order_rx,
            order_id_counter: Arc::new(RwLock::new(0)),
            order_id_to_stock_id_map,
            agent_channels: agent_channels_arc,
            shadow_update_tx,
            vip_shadow_update_tx,
            event_tx,
            last_traded_price,
            stock_market: stocks.clone(),
        }
    }

    pub fn get_stock_market_clone(&self) -> StockMarket {
        self.stock_market.clone()
    }

    /// Spawns the thread that processes all executed trades from all order books.
    fn spawn_trade_processor(
        trade_rx: Receiver<Trade>,
        agent_channels: Arc<HashMap<usize, AgentResponseChannels>>,
        event_tx: Sender<MarketEvent>,
        last_traded_price: Arc<RwLock<HashMap<u64, f64>>>,
        order_id_to_stock_id_map: Arc<RwLock<HashMap<u64, u64>>>,
    ) {
        thread::spawn(move || {
            let mut trade_count = 0;
            while let Ok(trade) = trade_rx.recv() {
                trade_count += 1;
                if trade_count % 10000 == 0 {
                    println!(
                        "[TradeProcessor] Processed {} trades at {:?}",
                        trade_count,
                        std::time::Instant::now()
                    );
                }
                // 1. Update the global last traded price
                last_traded_price
                    .write()
                    .unwrap()
                    .insert(trade.stock_id, trade.price as f64 / 100.0);

                // 2. Remove the maker order from the global map as it's now filled
                order_id_to_stock_id_map
                    .write()
                    .unwrap()
                    .remove(&trade.maker_order_id);

                // 3. Broadcast the trade to the central event bus
                event_tx
                    .send(MarketEvent::TradeOccurred(trade))
                    .unwrap_or_else(|e| {
                        eprintln!("[TradeProcessor] Failed to broadcast trade event: {}", e);
                    });

                // 4. Send the trade to the taker agent
                if let Some(taker_ch) = agent_channels.get(&trade.taker_agent_id) {
                    if taker_ch.trade_tx.send(trade).is_err() {
                        // Agent might have disconnected, log if necessary
                    }
                }

                // 5. Send the trade to the maker agent
                if let Some(maker_ch) = agent_channels.get(&trade.maker_agent_id) {
                    if maker_ch.trade_tx.send(trade).is_err() {
                        // Agent might have disconnected
                    }
                }
            }
        });
    }

    #[inline]
    fn next_order_id(&self) -> u64 {
        let mut counter = self.order_id_counter.write().unwrap();
        *counter += 1;
        *counter
    }

    fn process_request(&mut self, req: OrderRequest) {
        let order_id = self.next_order_id();

        match req {
            OrderRequest::LimitOrder {
                agent_id,
                stock_id,
                side,
                price,
                volume,
            } => {
                let order = Order {
                    id: order_id,
                    agent_id,
                    stock_id,
                    side,
                    price,
                    volume,
                    filled: 0,
                };
                // Store the mapping for future cancellations
                self.order_id_to_stock_id_map
                    .write()
                    .unwrap()
                    .insert(order_id, stock_id);

                if let Some(ch) = self.agent_channels.get(&agent_id) {
                    if ch.ack_tx.send(order).is_err() {
                        eprintln!(
                            "[Market] Agent disconnected, cannot send ack for order {}.",
                            order_id
                        );
                        return;
                    }
                }

                // Dispatch to Async Order Book
                if let Some(tx) = self.order_txs.get(&stock_id) {
                    if tx.send(req.clone()).is_err() {
                        eprintln!("[Market] Order book for stock {} is down.", stock_id);
                    }
                } else {
                    eprintln!("[Market] No order book for stock_id: {}", stock_id);
                }

                // Dispatch to Shadow Book (as before)
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
                let price = (self
                    .last_traded_price
                    .read()
                    .unwrap()
                    .get(&stock_id)
                    .copied()
                    .unwrap_or(150.0)
                    * 100.0)
                    .round() as u64;
                let order = Order {
                    id: order_id,
                    agent_id,
                    stock_id,
                    side,
                    volume,
                    price,
                    filled: 0,
                };
                // Market orders are immediately filled, so no need to map for cancellation.

                if let Some(ch) = self.agent_channels.get(&agent_id) {
                    if ch.ack_tx.send(order).is_err() {
                        eprintln!(
                            "[Market] Agent disconnected, cannot send ack for order {}.",
                            order_id
                        );
                        return;
                    }
                }

                // Dispatch to Async Order Book
                if let Some(tx) = self.order_txs.get(&stock_id) {
                    if tx.send(req.clone()).is_err() {
                        eprintln!("[Market] Order book for stock {} is down.", stock_id);
                    }
                } else {
                    eprintln!("[Market] No order book for stock_id: {}", stock_id);
                }

                // Dispatch to Shadow Book (as before)
                self.shadow_update_tx
                    .send(ShadowEvent::MarketOrder(order))
                    .unwrap();
                self.vip_shadow_update_tx
                    .send(ShadowEvent::MarketOrder(order))
                    .unwrap();
            }
            OrderRequest::CancelOrder { agent_id, order_id } => {
                // Find the stock_id for the order to be cancelled
                let stock_id_option = self
                    .order_id_to_stock_id_map
                    .read()
                    .unwrap()
                    .get(&order_id)
                    .copied();

                if let Some(stock_id) = stock_id_option {
                    // Dispatch to Async Order Book
                    if let Some(tx) = self.order_txs.get(&stock_id) {
                        if tx.send(req.clone()).is_err() {
                            eprintln!("[Market] Order book for stock {} is down.", stock_id);
                        }
                    } else {
                        eprintln!("[Market] No order book for stock_id: {}", stock_id);
                    }
                    // Remove from the map as it's being cancelled
                    self.order_id_to_stock_id_map
                        .write()
                        .unwrap()
                        .remove(&order_id);
                } else {
                    eprintln!(
                        "[Market] Attempted to cancel unknown or already filled order: {}",
                        order_id
                    );
                }

                // Dispatch to Shadow Book (as before)
                self.shadow_update_tx
                    .send(ShadowEvent::CancelOrder { order_id, agent_id })
                    .unwrap();
                self.vip_shadow_update_tx
                    .send(ShadowEvent::CancelOrder { order_id, agent_id })
                    .unwrap();
            }
        }
    }

    // Unchanged shadow worker logic
    fn spawn_shadow_worker(
        update_rx: Receiver<ShadowEvent>,
        shadow_book_handle: ShadowBookHandle,
        update_threshold: usize,
    ) {
        thread::spawn(move || {
            let mut back_buffer: MarketState = shadow_book_handle.read().unwrap().clone();
            let mut event_log: Vec<ShadowEvent> = Vec::with_capacity(update_threshold);
            let mut event_counter = 0;
            let mut shadow_trade_count = 0;

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
                    shadow_trade_count += 1;
                    if shadow_trade_count % 10000 == 0 {
                        println!(
                            "[ShadowWorker] Processed {} trades at {:?}",
                            shadow_trade_count,
                            std::time::Instant::now()
                        );
                    }
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
}

impl Marketable for Market {
    fn run(&mut self) {
        // The main market thread is now just a high-speed router.
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
