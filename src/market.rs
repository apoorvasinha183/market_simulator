// src/market.rs

use crate::Marketable;
use crate::events::MarketEvent;
use crate::simulation::orchestra::{AgentResponseChannels, ShadowEvent};
use crate::simulators::async_order_book::AsyncOrderBook;
use crate::stocks::definitions::StockMarket;
use crate::types::{Order, OrderRequest, Trade};
use crossbeam_channel::{Receiver, Sender, unbounded};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::SystemTime;

pub struct Market {
    order_txs: HashMap<u64, Sender<OrderRequest>>,
    order_rx: Receiver<OrderRequest>,
    order_id_counter: Arc<RwLock<u64>>,
    order_id_to_stock_id_map: Arc<RwLock<HashMap<u64, u64>>>,
    agent_channels: Arc<HashMap<usize, AgentResponseChannels>>,
    shadow_update_txs: HashMap<u64, Sender<ShadowEvent>>,
    vip_shadow_update_txs: HashMap<u64, Sender<ShadowEvent>>,
    #[allow(dead_code)]
    event_tx: Sender<MarketEvent>,
    pub last_traded_price: Arc<RwLock<HashMap<u64, f64>>>,
    stock_market: StockMarket,
}

impl Market {
    pub fn new(
        stocks: &StockMarket,
        order_rx: Receiver<OrderRequest>,
        agent_channels: HashMap<usize, AgentResponseChannels>,
        shadow_update_txs: HashMap<u64, Sender<ShadowEvent>>,
        vip_shadow_update_txs: HashMap<u64, Sender<ShadowEvent>>,
        event_tx: Sender<MarketEvent>,
    ) -> Self {
        let mut order_txs = HashMap::new();
        let (trade_tx, trade_rx) = unbounded::<Trade>();
        let mut last_traded_price_map = HashMap::new();

        for s in stocks.get_all_stocks() {
            last_traded_price_map.insert(s.id, s.initial_price);
        }
        let last_traded_price = Arc::new(RwLock::new(last_traded_price_map));

        for stock in stocks.get_all_stocks() {
            let (order_tx, stock_trade_rx) = AsyncOrderBook::new();
            order_txs.insert(stock.id, order_tx);

            let trade_tx_clone = trade_tx.clone();
            thread::spawn(move || {
                while let Ok(trade) = stock_trade_rx.recv() {
                    if trade_tx_clone.send(trade).is_err() {
                        break;
                    }
                }
            });
        }

        let agent_channels_arc = Arc::new(agent_channels);
        let order_id_to_stock_id_map = Arc::new(RwLock::new(HashMap::new()));

        Self::spawn_trade_processor(
            trade_rx,
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
            shadow_update_txs,
            vip_shadow_update_txs,
            event_tx,
            last_traded_price,
            stock_market: stocks.clone(),
        }
    }

    pub fn get_stock_market_clone(&self) -> StockMarket {
        self.stock_market.clone()
    }

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
                    let now = SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    println!(
                        "[{}] [Live Market] Processed 10,000 trades. Total: {}",
                        now, trade_count
                    );
                }

                last_traded_price
                    .write()
                    .unwrap()
                    .insert(trade.stock_id, trade.price as f64 / 100.0);

                order_id_to_stock_id_map
                    .write()
                    .unwrap()
                    .remove(&trade.maker_order_id);

                if event_tx.send(MarketEvent::TradeOccurred(trade)).is_err() {
                    eprintln!("[TradeProcessor] Failed to broadcast trade event");
                }

                if let Some(taker_ch) = agent_channels.get(&trade.taker_agent_id) {
                    taker_ch.trade_tx.send(trade).ok();
                }

                if let Some(maker_ch) = agent_channels.get(&trade.maker_agent_id) {
                    maker_ch.trade_tx.send(trade).ok();
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

        let (stock_id, shadow_event) = match &req {
            OrderRequest::LimitOrder {
                agent_id,
                stock_id,
                side,
                price,
                volume,
            } => {
                //println!("[Market] Received Limit Order: Agent {} Stock {} Side {:?} Price {} Volume {}", agent_id, stock_id, side, price, volume);
                let order = Order {
                    id: order_id,
                    agent_id: *agent_id,
                    stock_id: *stock_id,
                    side: *side,
                    price: *price,
                    volume: *volume,
                    filled: 0,
                };
                self.order_id_to_stock_id_map
                    .write()
                    .unwrap()
                    .insert(order_id, *stock_id);
                if let Some(ch) = self.agent_channels.get(agent_id) {
                    ch.ack_tx.send(order).ok();
                }
                (*stock_id, ShadowEvent::LimitOrder(order))
            }
            OrderRequest::MarketOrder {
                agent_id,
                stock_id,
                side,
                volume,
            } => {
                //println!("[Market] Received Market Order: Agent {} Stock {} Side {:?} Volume {}", agent_id, stock_id, side, volume);
                let price = (self
                    .last_traded_price
                    .read()
                    .unwrap()
                    .get(stock_id)
                    .copied()
                    .unwrap_or(150.0)
                    * 100.0)
                    .round() as u64;
                let order = Order {
                    id: order_id,
                    agent_id: *agent_id,
                    stock_id: *stock_id,
                    side: *side,
                    volume: *volume,
                    price,
                    filled: 0,
                };
                if let Some(ch) = self.agent_channels.get(agent_id) {
                    ch.ack_tx.send(order).ok();
                }
                (*stock_id, ShadowEvent::MarketOrder(order))
            }
            OrderRequest::CancelOrder { agent_id, order_id } => {
                //println!("[Market] Received Cancel Order: Agent {} Order {}", agent_id, order_id);

                let stock_id_lookup = {
                    let map_guard = self.order_id_to_stock_id_map.read().unwrap();
                    map_guard.get(order_id).copied()
                };

                if let Some(stock_id) = stock_id_lookup {
                    self.order_id_to_stock_id_map
                        .write()
                        .unwrap()
                        .remove(order_id);
                    (
                        stock_id,
                        ShadowEvent::CancelOrder {
                            order_id: *order_id,
                            agent_id: *agent_id,
                        },
                    )
                } else {
                    //println!("[Market] Attempted to cancel unknown or already filled order: {}", order_id);
                    return; // Order not found
                }
            }
        };

        // Dispatch to the appropriate AsyncOrderBook
        if let Some(tx) = self.order_txs.get(&stock_id) {
            //println!("[Market] Dispatching request to AsyncOrderBook for stock {}", stock_id);
            tx.send(req).ok();
        }

        // Dispatch to the appropriate shadow book channels
        if let Some(tx) = self.shadow_update_txs.get(&stock_id) {
            //println!("[Market] Dispatching shadow event for stock {}", stock_id);
            tx.send(shadow_event.clone()).ok();
        }
        if let Some(tx) = self.vip_shadow_update_txs.get(&stock_id) {
            //println!("[Market] Dispatching VIP shadow event for stock {}", stock_id);
            tx.send(shadow_event).ok();
        }
    }
}

impl Marketable for Market {
    fn run(&mut self) {
        while let Ok(req) = self.order_rx.recv() {
            self.process_request(req);
        }
    }

    // These methods are not relevant for the central market engine
    fn step(&mut self) -> f64 {
        0.0
    }
    fn current_price(&self) -> f64 {
        0.0
    }
    fn reset(&mut self) {}
    fn get_order_book(&self) -> Option<&crate::OrderBook> {
        None
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
