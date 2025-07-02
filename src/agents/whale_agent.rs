// src/agents/whale_agent.rs
use crossbeam_channel::{Receiver, Sender};
use rand::Rng;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;

use super::{
    agent_trait::{Agent, MarketView},
    config::{
        WHALE_ACTION_PROB, WHALE_ORDER_VOLUME, WHALE_PRICE_OFFSET_MAX, WHALE_PRICE_OFFSET_MIN,
        WHALE_REFRESH_THRESHOLD_BPS,
    },
    latency::WHALE_TICKS_UNTIL_ACTIVE,
};
use crate::{
    simulation::orchestra::ShadowBookHandle,
    types::order::{Order, OrderRequest, Side, Trade},
};

/// A patient, high-capital agent that periodically cancels and replaces
/// large limit orders far from mid-price.
#[allow(dead_code)]
#[derive(Clone)]
pub struct WhaleAgent {
    id: usize,
    // Communication and View Handles
    order_channel: Sender<OrderRequest>,
    ack_channel: Arc<Mutex<Receiver<Order>>>,
    port_channel: Arc<Mutex<Receiver<Trade>>>,
    view_handle: ShadowBookHandle,
    // State Handles
    inventory: Arc<RwLock<HashMap<u64, i64>>>,
    ticks_until_active: Arc<Mutex<u32>>,
    open_orders: Arc<RwLock<HashMap<u64, Order>>>,
    cash: Arc<RwLock<f64>>,
    margin: Arc<RwLock<f64>>,
    port_value: Arc<RwLock<f64>>,
    last_mid_prices: Arc<RwLock<HashMap<u64, u64>>>,
}

impl WhaleAgent {
    pub fn new(
        id: usize,
        order_channel: Sender<OrderRequest>,
        ack_channel: Receiver<Order>,
        port_channel: Receiver<Trade>,
        view_handle: ShadowBookHandle,
    ) -> Self {
        Self {
            id,
            order_channel,
            ack_channel: Arc::new(Mutex::new(ack_channel)),
            port_channel: Arc::new(Mutex::new(port_channel)),
            view_handle,
            inventory: Arc::new(RwLock::new(HashMap::new())), // Whales start with cash
            ticks_until_active: Arc::new(Mutex::new(WHALE_TICKS_UNTIL_ACTIVE)),
            open_orders: Arc::new(RwLock::new(HashMap::new())),
            cash: Arc::new(RwLock::new(1_000_000_000_000.0)),
            margin: Arc::new(RwLock::new(10_000_000_000_000.0)),
            port_value: Arc::new(RwLock::new(0.0)),
            last_mid_prices: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    // --- INTERNAL WORKER FUNCTIONS ---

    fn run_portfolio_updater_internal(
        port_rx: &Receiver<Trade>,
        inventory: &Arc<RwLock<HashMap<u64, i64>>>,
        cash: &Arc<RwLock<f64>>,
        open_orders: &Arc<RwLock<HashMap<u64, Order>>>,
        agent_id: usize,
    ) {
        while let Ok(tr) = port_rx.recv() {
            if tr.taker_agent_id == agent_id || tr.maker_agent_id == agent_id {
                let mut inventory_lock = inventory.write().unwrap();
                let mut cash_lock = cash.write().unwrap();
                let mut open_orders_lock = open_orders.write().unwrap();

                let vol_delta = if tr.taker_agent_id == agent_id {
                    if tr.taker_side == Side::Buy {
                        tr.volume as i64
                    } else {
                        -(tr.volume as i64)
                    }
                } else {
                    if tr.taker_side == Side::Sell {
                        tr.volume as i64
                    } else {
                        -(tr.volume as i64)
                    }
                };

                *inventory_lock.entry(tr.stock_id).or_insert(0) += vol_delta;
                *cash_lock -= vol_delta as f64 * (tr.price as f64 / 100.0);

                if tr.maker_agent_id == agent_id {
                    if let Some(o) = open_orders_lock.get_mut(&tr.maker_order_id) {
                        o.filled += tr.volume;
                        if o.filled >= o.volume {
                            open_orders_lock.remove(&tr.maker_order_id);
                        }
                    }
                }
            }
        }
    }

    fn run_ack_listener_internal(
        ack_rx: &Receiver<Order>,
        open_orders: &Arc<RwLock<HashMap<u64, Order>>>,
    ) {
        while let Ok(order) = ack_rx.recv() {
            open_orders.write().unwrap().insert(order.id, order);
        }
    }

    fn decide_actions_internal(
        id: usize,
        ticks_until_active: &Arc<Mutex<u32>>,
        open_orders: &Arc<RwLock<HashMap<u64, Order>>>,
        view_handle: &ShadowBookHandle,
        order_channel: &Sender<OrderRequest>,
        last_mid_prices: &Arc<RwLock<HashMap<u64, u64>>>,
    ) {
        {
            let mut ticks = ticks_until_active.lock().unwrap();
            if *ticks > 0 {
                *ticks -= 1;
                return;
            }
        }

        let view = view_handle.read().unwrap();
        let ids: Vec<u64> = view.stocks.get_all_ids();
        if ids.is_empty() {
            return;
        }

        let mid_prices: HashMap<u64, u64> = ids
            .iter()
            .filter_map(|id| view.get_mid_price(*id).map(|price| (*id, price)))
            .collect();

        thread::scope(|s| {
            for &stock_id in &ids {
                let order_channel = order_channel.clone();
                let open_orders = open_orders.clone();
                let last_mid_prices = last_mid_prices.clone();
                let current_mid_price = *mid_prices.get(&stock_id).unwrap_or(&0);

                s.spawn(move || {
                    let mut rng = rand::thread_rng();
                    if !rng.gen_bool(WHALE_ACTION_PROB) {
                        return;
                    }

                    if current_mid_price > 0 {
                        let mut last_prices = last_mid_prices.write().unwrap();
                        let last_price = last_prices.entry(stock_id).or_insert(current_mid_price);

                        let price_diff_bps = (current_mid_price as i64 - *last_price as i64).abs()
                            as f64
                            / *last_price as f64
                            * 10000.0;

                        if price_diff_bps > WHALE_REFRESH_THRESHOLD_BPS as f64 {
                            // Full reset
                            let mut open_orders_lock = open_orders.write().unwrap();
                            let orders_to_cancel: Vec<u64> = open_orders_lock
                                .values()
                                .filter(|o| o.stock_id == stock_id)
                                .map(|o| o.id)
                                .collect();

                            for order_id in orders_to_cancel {
                                order_channel
                                    .send(OrderRequest::CancelOrder {
                                        agent_id: id,
                                        order_id,
                                    })
                                    .expect("Failed to send cancel order");
                                open_orders_lock.remove(&order_id);
                            }

                            // Place new orders
                            let buy_bias =
                                rng.gen_range(WHALE_PRICE_OFFSET_MIN..=WHALE_PRICE_OFFSET_MAX);
                            let sell_bias =
                                rng.gen_range(WHALE_PRICE_OFFSET_MIN..=WHALE_PRICE_OFFSET_MAX);
                            let bid_px = crate::agents::quantize_price(
                                current_mid_price.saturating_sub(buy_bias),
                            );
                            let ask_px = crate::agents::quantize_price(
                                current_mid_price.saturating_add(sell_bias),
                            );

                            order_channel
                                .send(OrderRequest::LimitOrder {
                                    agent_id: id,
                                    stock_id,
                                    side: Side::Buy,
                                    price: bid_px,
                                    volume: WHALE_ORDER_VOLUME,
                                })
                                .expect("Failed to send whale limit order");

                            order_channel
                                .send(OrderRequest::LimitOrder {
                                    agent_id: id,
                                    stock_id,
                                    side: Side::Sell,
                                    price: ask_px,
                                    volume: WHALE_ORDER_VOLUME,
                                })
                                .expect("Failed to send whale limit order");

                            *last_price = current_mid_price;
                        } else {
                            // Partial refresh
                            let open_orders_lock = open_orders.read().unwrap();
                            let has_bids = open_orders_lock
                                .values()
                                .any(|o| o.stock_id == stock_id && o.side == Side::Buy);
                            let has_asks = open_orders_lock
                                .values()
                                .any(|o| o.stock_id == stock_id && o.side == Side::Sell);

                            if !has_bids {
                                let buy_bias =
                                    rng.gen_range(WHALE_PRICE_OFFSET_MIN..=WHALE_PRICE_OFFSET_MAX);
                                let bid_px = crate::agents::quantize_price(
                                    current_mid_price.saturating_sub(buy_bias),
                                );
                                order_channel
                                    .send(OrderRequest::LimitOrder {
                                        agent_id: id,
                                        stock_id,
                                        side: Side::Buy,
                                        price: bid_px,
                                        volume: WHALE_ORDER_VOLUME,
                                    })
                                    .expect("Failed to send whale limit order");
                            }

                            if !has_asks {
                                let sell_bias =
                                    rng.gen_range(WHALE_PRICE_OFFSET_MIN..=WHALE_PRICE_OFFSET_MAX);
                                let ask_px = crate::agents::quantize_price(
                                    current_mid_price.saturating_add(sell_bias),
                                );
                                order_channel
                                    .send(OrderRequest::LimitOrder {
                                        agent_id: id,
                                        stock_id,
                                        side: Side::Sell,
                                        price: ask_px,
                                        volume: WHALE_ORDER_VOLUME,
                                    })
                                    .expect("Failed to send whale limit order");
                            }
                        }
                    }
                });
            }
        });
    }
}

// -----------------------------------------------------------------------------
//  Agent impl
// -----------------------------------------------------------------------------
impl Agent for WhaleAgent {
    fn run(&mut self) {
        let portfolio_rx_handle = self.port_channel.clone();
        let ack_rx_handle = self.ack_channel.clone();
        let inventory_handle = self.inventory.clone();
        let cash_handle = self.cash.clone();
        let open_orders_handle_for_portfolio = self.open_orders.clone();
        let open_orders_handle_for_acks = self.open_orders.clone();
        let agent_id = self.id;

        thread::spawn(move || {
            let rx = portfolio_rx_handle.lock().unwrap();
            Self::run_portfolio_updater_internal(
                &rx,
                &inventory_handle,
                &cash_handle,
                &open_orders_handle_for_portfolio,
                agent_id,
            );
        });

        thread::spawn(move || {
            let rx = ack_rx_handle.lock().unwrap();
            Self::run_ack_listener_internal(&rx, &open_orders_handle_for_acks);
        });

        loop {
            self.decide_actions();
            thread::sleep(std::time::Duration::from_millis(100)); // Whales act less frequently
        }
    }

    fn decide_actions(&mut self) {
        Self::decide_actions_internal(
            self.id,
            &self.ticks_until_active,
            &self.open_orders,
            &self.view_handle,
            &self.order_channel,
            &self.last_mid_prices,
        );
    }

    fn buy_stock(&mut self, stock_id: u64, volume: u64) {
        self.order_channel
            .send(OrderRequest::MarketOrder {
                agent_id: self.id,
                stock_id,
                side: Side::Buy,
                volume,
            })
            .expect("Failed to send buy order");
    }

    fn sell_stock(&mut self, stock_id: u64, volume: u64) {
        self.order_channel
            .send(OrderRequest::MarketOrder {
                agent_id: self.id,
                stock_id,
                side: Side::Sell,
                volume,
            })
            .expect("Failed to send sell order");
    }

    fn margin_call(&mut self) {
        // Whales have infinite capital and are never margin called.
    }

    /* bookkeeping */
    fn acknowledge_order(&mut self) {
        let rx = self.ack_channel.lock().unwrap();
        while let Ok(order) = rx.try_recv() {
            self.open_orders.write().unwrap().insert(order.id, order);
        }
    }

    fn update_portfolio(&mut self) {
        let rx = self.port_channel.lock().unwrap();
        while let Ok(tr) = rx.try_recv() {
            if tr.taker_agent_id == self.id || tr.maker_agent_id == self.id {
                let mut inventory_lock = self.inventory.write().unwrap();
                let mut cash_lock = self.cash.write().unwrap();
                let mut open_orders_lock = self.open_orders.write().unwrap();

                let vol_delta = if tr.taker_agent_id == self.id {
                    if tr.taker_side == Side::Buy {
                        tr.volume as i64
                    } else {
                        -(tr.volume as i64)
                    }
                } else {
                    if tr.taker_side == Side::Sell {
                        tr.volume as i64
                    } else {
                        -(tr.volume as i64)
                    }
                };

                *inventory_lock.entry(tr.stock_id).or_insert(0) += vol_delta;
                *cash_lock -= vol_delta as f64 * (tr.price as f64 / 100.0);
                if tr.maker_agent_id == self.id {
                    if let Some(o) = open_orders_lock.get_mut(&tr.maker_order_id) {
                        o.filled += tr.volume;
                        if o.filled >= o.volume {
                            open_orders_lock.remove(&tr.maker_order_id);
                        }
                    }
                }
            }
        }
    }

    fn get_pending_orders(&self) -> Vec<Order> {
        self.open_orders.read().unwrap().values().cloned().collect()
    }

    fn cancel_open_order(&mut self, id: u64) {
        // This would now send a request rather than just modifying the internal map.
        if self.open_orders.write().unwrap().remove(&id).is_some() {
            self.order_channel
                .send(OrderRequest::CancelOrder {
                    agent_id: self.id,
                    order_id: id,
                })
                .expect("Failed to send cancel order request");
        }
    }

    /* misc getters */
    fn get_id(&self) -> usize {
        self.id
    }

    fn get_inventory(&self) -> i64 {
        self.inventory.read().unwrap().values().sum()
    }

    fn clone_agent(&self) -> Box<dyn Agent> {
        Box::new(self.clone())
    }

    fn evaluate_port(&mut self, view: &MarketView) -> f64 {
        let inventory_lock = self.inventory.read().unwrap();
        let new_port_value = inventory_lock.iter().fold(0.0, |acc, (stock_id, &vol)| {
            if let Some(px) = view.get_mid_price(*stock_id) {
                acc + (vol as f64 * (px as f64 / 100.0))
            } else {
                acc
            }
        });
        *self.port_value.write().unwrap() = new_port_value;
        new_port_value
    }
}
