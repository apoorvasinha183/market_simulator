// src/agents/market_maker_agent.rs
use super::{
    agent_trait::{Agent, MarketView},
    config::{
        MM_DESIRED_SPREAD, MM_INITIAL_CENTER_PRICE, MM_QUOTE_VOL_MAX,
        MM_QUOTE_VOL_MIN, MM_SEED_DECAY, MM_SEED_DEPTH_PCT, MM_SEED_LEVELS, MM_SEED_TICK_SPACING,
        MM_SKEW_FACTOR, MM_UNSTICK_VOL_MAX, MM_UNSTICK_VOL_MIN,
    },
};
use crate::{
    agents::latency::MM_TICKS_UNTIL_ACTIVE,
    simulation::orchestra::ShadowBookHandle,
    types::order::{Order, OrderRequest, Side, Trade},
};
use crossbeam_channel::{Receiver, Sender};
use rand::Rng;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;

/* guard-rails */
const MIN_PRICE: u64 = 1_00; // $1.00
const MAX_PRICE: u64 = 3_000_00; // $3,000.00
#[inline]
fn clamp(p: i128) -> u64 {
    p.max(MIN_PRICE as i128).min(MAX_PRICE as i128) as u64
}

#[derive(Clone)]
pub struct MarketMakerAgent {
    id: usize,
    // Communication and View Handles
    order_channel: Sender<OrderRequest>,
    ack_channel: Arc<Mutex<Receiver<Order>>>,
    port_channel: Arc<Mutex<Receiver<Trade>>>,
    view_handle: ShadowBookHandle,
    // State Handles
    inventory: Arc<RwLock<HashMap<u64, i64>>>,
    ticks_until_active: Arc<Mutex<u32>>,
    bootstrapped: Arc<RwLock<HashMap<u64, bool>>>,
    open_orders: Arc<RwLock<HashMap<u64, Order>>>,
    cash: Arc<RwLock<f64>>,
    margin: Arc<RwLock<f64>>,
    port_value: Arc<RwLock<f64>>,
}

impl MarketMakerAgent {
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
            ticks_until_active: Arc::new(Mutex::new(MM_TICKS_UNTIL_ACTIVE)),
            bootstrapped: Arc::new(RwLock::new(HashMap::new())),
            open_orders: Arc::new(RwLock::new(HashMap::new())),
            cash: Arc::new(RwLock::new(100_000_000_000.0)),
            margin: Arc::new(RwLock::new(400_000_000_000.0)),
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
                    if tr.taker_side == Side::Buy { tr.volume as i64 } else { -(tr.volume as i64) }
                } else { // This agent was the maker
                    if tr.taker_side == Side::Sell { tr.volume as i64 } else { -(tr.volume as i64) }
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
        inventory: &Arc<RwLock<HashMap<u64, i64>>>,
        bootstrapped: &Arc<RwLock<HashMap<u64, bool>>>,
        view_handle: &ShadowBookHandle,
        order_channel: &Sender<OrderRequest>,
    ) {
        {
            let mut ticks = ticks_until_active.lock().unwrap();
            if *ticks > 0 { *ticks -= 1; return; }
        }

        let view = view_handle.read().unwrap();
        let ids: Vec<u64> = view.stocks.get_all_ids();
        if ids.is_empty() { return; }

        let mut handles = Vec::new();

        for &stock_id in &ids {
            let order_channel_clone = order_channel.clone();
            let inventory_clone = inventory.clone();
            let bootstrapped_clone = bootstrapped.clone();
            let book_clone = view.book(stock_id).cloned();
            let initial_price = view.stocks.get_stock_by_id(stock_id)
                .map(|s| (s.initial_price * 100.0) as u64)
                .unwrap_or(MM_INITIAL_CENTER_PRICE);

            let handle = thread::spawn(move || {
                let book = match book_clone { Some(b) => b, None => return };
                let is_bootstrapped = *bootstrapped_clone.read().unwrap().get(&stock_id).unwrap_or(&false);
                let agent_id = id; // move id into thread

                if !is_bootstrapped {
                    let total_inventory: i64 = 1_000_000; // MM starting inventory for seeding
                    let side_budget = (total_inventory.abs() as f64 * MM_SEED_DEPTH_PCT) as u64;
                    let mut vol_at_lvl = (side_budget as f64 * (1.0 - MM_SEED_DECAY)
                        / (1.0 - MM_SEED_DECAY.powi(MM_SEED_LEVELS as i32)))
                        as u64;

                    for lvl in 0..MM_SEED_LEVELS {
                        let vol = vol_at_lvl;
                        vol_at_lvl = (vol_at_lvl as f64 * MM_SEED_DECAY) as u64;

                        let bid_px = clamp(initial_price as i128 - (MM_DESIRED_SPREAD / 2 + lvl as u64 * MM_SEED_TICK_SPACING) as i128);
                        let ask_px = clamp(initial_price as i128 + (MM_DESIRED_SPREAD / 2 + lvl as u64 * MM_SEED_TICK_SPACING) as i128);

                        order_channel_clone.send(OrderRequest::LimitOrder { agent_id, stock_id, side: Side::Buy, price: bid_px, volume: vol }).unwrap();
                        order_channel_clone.send(OrderRequest::LimitOrder { agent_id, stock_id, side: Side::Sell, price: ask_px, volume: vol }).unwrap();
                    }
                    bootstrapped_clone.write().unwrap().insert(stock_id, true);
                } else {
                    let best_bid = book.bids.keys().next_back().copied();
                    let best_ask = book.asks.keys().next().copied();

                    if let (Some(bid), None) = (best_bid, best_ask) {
                        let ask_px = clamp(bid as i128 + 1);
                        let vol = rand::thread_rng().gen_range(MM_UNSTICK_VOL_MIN..=MM_UNSTICK_VOL_MAX);
                        order_channel_clone.send(OrderRequest::LimitOrder { agent_id, stock_id, side: Side::Sell, price: ask_px, volume: vol }).unwrap();
                    } else if let (None, Some(ask)) = (best_bid, best_ask) {
                        let bid_px = clamp(ask as i128 - 1);
                        let vol = rand::thread_rng().gen_range(MM_UNSTICK_VOL_MIN..=MM_UNSTICK_VOL_MAX);
                        order_channel_clone.send(OrderRequest::LimitOrder { agent_id, stock_id, side: Side::Buy, price: bid_px, volume: vol }).unwrap();
                    } else {
                        let center = match (best_bid, best_ask) {
                            (Some(b), Some(a)) if a > b => ((b as u128 + a as u128) / 2) as u64,
                            (None, None) => MM_INITIAL_CENTER_PRICE,
                            _ => return,
                        };

                        let current_inventory = *inventory_clone.read().unwrap().get(&stock_id).unwrap_or(&0);
                        let inventory_skew = (current_inventory as f64 * MM_SKEW_FACTOR) as i64;
                        let our_center = clamp(center as i128 - inventory_skew as i128);
                        let bid_px = clamp(our_center as i128 - (MM_DESIRED_SPREAD / 2) as i128);
                        let ask_px = clamp(our_center as i128 + (MM_DESIRED_SPREAD / 2) as i128);

                        if ask_px > bid_px && !best_ask.map_or(false, |a| bid_px >= a) && !best_bid.map_or(false, |b| ask_px <= b) {
                            let vol = rand::thread_rng().gen_range(MM_QUOTE_VOL_MIN..=MM_QUOTE_VOL_MAX);
                            order_channel_clone.send(OrderRequest::LimitOrder { agent_id, stock_id, side: Side::Buy, price: bid_px, volume: vol }).unwrap();
                            order_channel_clone.send(OrderRequest::LimitOrder { agent_id, stock_id, side: Side::Sell, price: ask_px, volume: vol }).unwrap();
                        }
                    }
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }
}

// -----------------------------------------------------------------------------
//  Agent impl
// -----------------------------------------------------------------------------
impl Agent for MarketMakerAgent {
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
            Self::run_portfolio_updater_internal(&rx, &inventory_handle, &cash_handle, &open_orders_handle_for_portfolio, agent_id);
        });

        thread::spawn(move || {
            let rx = ack_rx_handle.lock().unwrap();
            Self::run_ack_listener_internal(&rx, &open_orders_handle_for_acks);
        });

        loop {
            self.decide_actions();
            thread::sleep(std::time::Duration::from_millis(50));
        }
    }

    fn decide_actions(&mut self) {
        Self::decide_actions_internal(
            self.id,
            &self.ticks_until_active,
            &self.inventory,
            &self.bootstrapped,
            &self.view_handle,
            &self.order_channel,
        );
    }

    fn buy_stock(&mut self, stock_id: u64, volume: u64) {
        self.order_channel.send(OrderRequest::MarketOrder { agent_id: self.id, stock_id, side: Side::Buy, volume }).expect("Failed to send buy order");
    }

    fn sell_stock(&mut self, stock_id: u64, volume: u64) {
        self.order_channel.send(OrderRequest::MarketOrder { agent_id: self.id, stock_id, side: Side::Sell, volume }).expect("Failed to send sell order");
    }

    fn margin_call(&mut self) {
        let cash = *self.cash.read().unwrap();
        let margin = *self.margin.read().unwrap();
        if cash <= -margin {
            let inventory = self.inventory.read().unwrap();
            for (&stock_id, &vol) in inventory.iter() {
                if vol > 0 { 
                    self.order_channel.send(OrderRequest::MarketOrder { agent_id: self.id, stock_id, side: Side::Sell, volume:vol.unsigned_abs() }).expect("Failed to send sell order");
                 }
                else if vol < 0 { 
                    self.order_channel.send(OrderRequest::MarketOrder { agent_id: self.id, stock_id, side: Side::Buy, volume: vol.unsigned_abs() }).expect("Failed to send buy order");
                 }
            }
        }
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
                    if tr.taker_side == Side::Buy { tr.volume as i64 } else { -(tr.volume as i64) }
                } else {
                    if tr.taker_side == Side::Sell { tr.volume as i64 } else { -(tr.volume as i64) }
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
            } else { acc }
        });
        *self.port_value.write().unwrap() = new_port_value;
        new_port_value
    }
}

