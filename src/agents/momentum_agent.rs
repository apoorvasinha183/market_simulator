// src/agents/momentum_agent.rs

use super::agent_trait::Agent;
use super::config::{
    MOMENTUM_AGENT_ACTION_PROB, MOMENTUM_AGENT_MOMENTUM_THRESHOLD, MOMENTUM_AGENT_MOMENTUM_WINDOW,
    MOMENTUM_AGENT_PRICE_OFFSET_MAX, MOMENTUM_AGENT_PRICE_OFFSET_MIN, MOMENTUM_AGENT_VOL_MAX,
    MOMENTUM_AGENT_VOL_MIN,
};
use crate::simulation::orchestra::{MarketState, ShadowBookHandle};
use crate::types::order::{Order, OrderRequest, Side, Trade};
use crossbeam_channel::{Receiver, Sender};
use rand::Rng;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;

pub struct MomentumAgent {
    id: usize,
    order_channel: Sender<OrderRequest>,
    ack_channel: Arc<Mutex<Receiver<Order>>>,
    port_channel: Arc<Mutex<Receiver<Trade>>>,
    view_handle: ShadowBookHandle,
    price_history: Arc<RwLock<HashMap<u64, VecDeque<f64>>>>,
    open_orders: Arc<RwLock<HashMap<u64, Order>>>,
    cash: Arc<RwLock<f64>>,
    inventory: Arc<RwLock<HashMap<u64, i64>>>,
}

impl MomentumAgent {
    pub fn new(
        id: usize,
        order_channel: Sender<OrderRequest>,
        ack_channel: Receiver<Order>,
        port_channel: Receiver<Trade>,
        view_handle: ShadowBookHandle,
    ) -> Self {
        Self::new_with_inventory(id, order_channel, ack_channel, port_channel, view_handle, None)
    }

    pub fn new_with_inventory(
        id: usize,
        order_channel: Sender<OrderRequest>,
        ack_channel: Receiver<Order>,
        port_channel: Receiver<Trade>,
        view_handle: ShadowBookHandle,
        initial_inventory: Option<HashMap<u64, u64>>, // stock_id -> shares
    ) -> Self {
        // Convert u64 shares to i64 positions (positive = long)
        let inventory = if let Some(inv) = initial_inventory {
            inv.into_iter().map(|(k, v)| (k, v as i64)).collect()
        } else {
            HashMap::new()
        };

        Self {
            id,
            order_channel,
            ack_channel: Arc::new(Mutex::new(ack_channel)),
            port_channel: Arc::new(Mutex::new(port_channel)),
            view_handle,
            price_history: Arc::new(RwLock::new(HashMap::new())),
            open_orders: Arc::new(RwLock::new(HashMap::new())),
            cash: Arc::new(RwLock::new(1_000_000.0)), // Starting cash
            inventory: Arc::new(RwLock::new(inventory)),
        }
    }

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
        order_channel: &Sender<OrderRequest>,
        view_handle: &ShadowBookHandle,
        price_history: &Arc<RwLock<HashMap<u64, VecDeque<f64>>>>,
        cash: &Arc<RwLock<f64>>,
    ) {
        let view = view_handle.read().unwrap();
        let stock_ids: Vec<u64> = view.stocks.get_all_ids();
        if stock_ids.is_empty() {
            return;
        }

        let mut rng = rand::thread_rng();

        for &stock_id in &stock_ids {
            if !rng.gen_bool(MOMENTUM_AGENT_ACTION_PROB) {
                continue;
            }

            let current_price_f64 = match view.last_traded_price.get(&stock_id) {
                Some(&price) => price,
                None => continue, // Skip if no trade has occurred yet
            };
            let current_price_cents = (current_price_f64 * 100.0).round() as u64;

            let mut history_lock = price_history.write().unwrap();
            let history = history_lock
                .entry(stock_id)
                .or_insert_with(|| VecDeque::with_capacity(MOMENTUM_AGENT_MOMENTUM_WINDOW));

            if history.len() == MOMENTUM_AGENT_MOMENTUM_WINDOW {
                history.pop_front();
            }
            history.push_back(current_price_f64);

            if history.len() < MOMENTUM_AGENT_MOMENTUM_WINDOW {
                continue; // Not enough data for momentum calculation
            }

            let first_price = history.front().unwrap();
            let last_price = history.back().unwrap();

            let price_change_pct = (last_price - first_price) / first_price;

            let side = if price_change_pct > MOMENTUM_AGENT_MOMENTUM_THRESHOLD {
                // Positive momentum, buy
                Side::Buy
            } else if price_change_pct < -MOMENTUM_AGENT_MOMENTUM_THRESHOLD {
                // Negative momentum, sell
                Side::Sell
            } else {
                continue; // No significant momentum
            };

            let offset =
                rng.gen_range(MOMENTUM_AGENT_PRICE_OFFSET_MIN..=MOMENTUM_AGENT_PRICE_OFFSET_MAX);
            let limit_price = match side {
                Side::Buy => current_price_cents.saturating_add(offset),
                Side::Sell => current_price_cents.saturating_sub(offset),
            };
            let limit_price = crate::agents::quantize_price(limit_price);

            let volume = rng.gen_range(MOMENTUM_AGENT_VOL_MIN..=MOMENTUM_AGENT_VOL_MAX);

            // Basic cash check for buy orders
            if side == Side::Buy {
                let cost = volume as f64 * (limit_price as f64 / 100.0);
                if *cash.read().unwrap() < cost {
                    continue; // Not enough cash
                }
            }

            let order_req = OrderRequest::LimitOrder {
                order_id: 0,
                agent_id: id,
                stock_id,
                side,
                price: limit_price,
                volume,
            };

            if let Err(e) = order_channel.send(order_req) {
                eprintln!(
                    "[MomentumAgent {}] Failed to send order for stock {}: {}",
                    id, stock_id, e
                );
            }
        }
    }
}

impl Agent for MomentumAgent {
    fn run(&mut self) {
        let portfolio_rx_handle = self.port_channel.clone();
        let ack_rx_handle = self.ack_channel.clone();
        let inventory_handle = self.inventory.clone();
        let cash_handle = self.cash.clone();
        let open_orders_handle_1 = self.open_orders.clone();
        let open_orders_handle_2 = self.open_orders.clone();
        let agent_id = self.id;
        // make it sleep for 10 seconds
        thread::sleep(std::time::Duration::from_secs(10));
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
            thread::sleep(std::time::Duration::from_nanos(100)); // Act less frequently
        }
    }

    fn decide_actions(&mut self) {
        Self::decide_actions_internal(
            self.id,
            &self.order_channel,
            &self.view_handle,
            &self.price_history,
            &self.cash,
        );
    }

    fn buy_stock(&mut self, stock_id: u64, volume: u64) {
        self.order_channel
            .send(OrderRequest::MarketOrder {
                order_id: 0,
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
                order_id: 0,
                agent_id: self.id,
                stock_id,
                side: Side::Sell,
                volume,
            })
            .expect("Failed to send sell order");
    }

    fn margin_call(&mut self) {
        let current_cash = *self.cash.read().unwrap();
        // Simple margin call: if cash is negative, try to sell some inventory
        if current_cash < 0.0 {
            let inventory_lock = self.inventory.read().unwrap();
            for (&stock_id, &vol) in inventory_lock.iter() {
                if vol > 0 {
                    self.order_channel
                        .send(OrderRequest::MarketOrder {
                            order_id: 0,
                            agent_id: self.id,
                            stock_id,
                            side: Side::Sell,
                            volume: vol.unsigned_abs(),
                        })
                        .expect("Failed to send liquidation order");
                }
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
            if tr.maker_agent_id == self.id || tr.taker_agent_id == self.id {
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
        self.open_orders.write().unwrap().remove(&id);
        self.order_channel
            .send(OrderRequest::CancelOrder {
                agent_id: self.id,
                order_id: id,
            })
            .expect("Failed to send cancel order request");
    }

    fn get_id(&self) -> usize {
        self.id
    }

    fn get_inventory(&self) -> i64 {
        self.inventory.read().unwrap().values().sum()
    }

    fn clone_agent(&self) -> Box<dyn Agent> {
        Box::new(MomentumAgent {
            id: self.id,
            order_channel: self.order_channel.clone(),
            ack_channel: self.ack_channel.clone(),
            port_channel: self.port_channel.clone(),
            view_handle: self.view_handle.clone(),
            price_history: self.price_history.clone(),
            open_orders: self.open_orders.clone(),
            cash: self.cash.clone(),
            inventory: self.inventory.clone(),
        })
    }

    fn evaluate_port(&mut self, view: &MarketState) -> f64 {
        let inventory_lock = self.inventory.read().unwrap();
        let new_port_value = inventory_lock.iter().fold(0.0, |acc, (stock_id, &vol)| {
            if let Some(px) = view.get_mid_price(*stock_id) {
                acc + vol as f64 * (px as f64 / 100.0)
            } else {
                acc
            }
        });
        *self.cash.write().unwrap() + new_port_value
    }
}
