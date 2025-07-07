// src/agents/dumb_limit_agent.rs

use crossbeam_channel::{Receiver, Sender};
use rand::Rng;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;

use super::agent_trait::Agent;
use super::config::{
    LIMIT_AGENT_ACTION_PROB, LIMIT_AGENT_MAX_OFFSET, LIMIT_AGENT_NUM_TRADERS, LIMIT_AGENT_VOL_MAX,
    LIMIT_AGENT_VOL_MIN,
};
use crate::simulation::orchestra::{MarketState, ShadowBookHandle};
use crate::{
    agents::latency::LIMIT_AGENT_TICKS_UNTIL_ACTIVE,
    types::order::{Order, OrderRequest, Side, Trade},
};

#[allow(dead_code)]
#[derive(Clone)]
pub struct DumbLimitAgent {
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
}

impl DumbLimitAgent {
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
            inventory: Arc::new(RwLock::new(HashMap::new())),
            ticks_until_active: Arc::new(Mutex::new(LIMIT_AGENT_TICKS_UNTIL_ACTIVE)),
            open_orders: Arc::new(RwLock::new(HashMap::new())),
            cash: Arc::new(RwLock::new(100_000_000.0)),
            margin: Arc::new(RwLock::new(10_000_000_000.0)),
            port_value: Arc::new(RwLock::new(0.0)),
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
                    // maker_agent_id == agent_id
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
            let mut open_orders_lock = open_orders.write().unwrap();
            open_orders_lock.insert(order.id, order);
        }
    }

    fn decide_actions_internal(
        id: usize,
        ticks_until_active: &Arc<Mutex<u32>>,
        view_handle: &ShadowBookHandle,
        order_channel: &Sender<OrderRequest>,
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

        thread::scope(|s| {
            for &stock_id in &ids {
                let order_channel = order_channel.clone();
                // Use last traded price as the reference for placing orders.
                let last_price = match view.last_traded_price.get(&stock_id) {
                    Some(&price) => (price * 100.0) as u64, // Convert to cents
                    None => continue,                       // Skip if no trade has occurred yet
                };

                s.spawn(move || {
                    let mut rng = rand::thread_rng();
                    for _ in 0..LIMIT_AGENT_NUM_TRADERS {
                        if !rng.gen_bool(LIMIT_AGENT_ACTION_PROB) {
                            continue;
                        }

                        let side = if rng.gen_bool(0.5) {
                            Side::Buy
                        } else {
                            Side::Sell
                        };
                        let offset = rng.gen_range(1..=LIMIT_AGENT_MAX_OFFSET);

                        let price = match side {
                            Side::Buy => last_price.saturating_sub(offset),
                            Side::Sell => last_price.saturating_add(offset),
                        };
                        let price = crate::agents::quantize_price(price);

                        let volume = rng.gen_range(LIMIT_AGENT_VOL_MIN..=LIMIT_AGENT_VOL_MAX);

                        if rng.gen_bool(0.01) {
                            // Chance to submit a market order
                            order_channel
                                .send(OrderRequest::MarketOrder {
                                    agent_id: id,
                                    stock_id,
                                    side,
                                    volume,
                                })
                                .expect("Failed to send market order");
                        } else {
                            order_channel
                                .send(OrderRequest::LimitOrder {
                                    agent_id: id,
                                    stock_id,
                                    side,
                                    price,
                                    volume,
                                })
                                .expect("Failed to send limit order");
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
impl Agent for DumbLimitAgent {
    fn run(&mut self) {
        let portfolio_rx_handle = self.port_channel.clone();
        let ack_rx_handle = self.ack_channel.clone();
        let inventory_handle = self.inventory.clone();
        let cash_handle = self.cash.clone();
        let open_orders_handle_1 = self.open_orders.clone();
        let open_orders_handle_2 = self.open_orders.clone();
        let agent_id = self.id;

        thread::spawn(move || {
            let rx = portfolio_rx_handle.lock().unwrap();
            Self::run_portfolio_updater_internal(
                &rx,
                &inventory_handle,
                &cash_handle,
                &open_orders_handle_1,
                agent_id,
            );
        });

        thread::spawn(move || {
            let rx = ack_rx_handle.lock().unwrap();
            Self::run_ack_listener_internal(&rx, &open_orders_handle_2);
        });

        loop {
            self.decide_actions();
            thread::sleep(std::time::Duration::from_micros(10));
        }
    }

    fn decide_actions(&mut self) {
        Self::decide_actions_internal(
            self.id,
            &self.ticks_until_active,
            &self.view_handle,
            &self.order_channel,
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
                side: Side::Sell,
                stock_id,
                volume,
            })
            .expect("Failed to send sell order");
    }

    fn margin_call(&mut self) {
        let inventory_lock = self.inventory.read().unwrap();
        for (&stock_id, &volume) in inventory_lock.iter() {
            // If we are short on any stock, buy to cover the position.
            if volume < 0 {
                self.order_channel
                    .send(OrderRequest::MarketOrder {
                        agent_id: self.id,
                        stock_id,
                        side: Side::Buy,
                        volume: volume.unsigned_abs(),
                    })
                    .expect("Failed to send buy order");
            }
        }
    }

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
                    // maker_agent_id == self.id
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
        self.open_orders.write().unwrap().remove(&id);
    }

    fn get_id(&self) -> usize {
        self.id
    }

    fn get_inventory(&self) -> i64 {
        self.inventory.read().unwrap().values().sum()
    }

    fn clone_agent(&self) -> Box<dyn Agent> {
        Box::new(self.clone())
    }

    fn evaluate_port(&mut self, view: &MarketState) -> f64 {
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
