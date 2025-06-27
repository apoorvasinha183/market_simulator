// src/agents/dumb_agent.rs
use rand::{Rng, seq::SliceRandom};
use std::collections::HashMap;
use crossbeam_channel::{Receiver, Sender};
use std::sync::{Arc, RwLock, Mutex};
use std::thread;
use super::{
    agent_trait::{Agent, MarketView},
    config::{
        DUMB_AGENT_ACTION_PROB, DUMB_AGENT_LARGE_VOL_CHANCE, DUMB_AGENT_LARGE_VOL_MAX,
        DUMB_AGENT_LARGE_VOL_MIN, DUMB_AGENT_NUM_TRADERS, DUMB_AGENT_TYPICAL_VOL_MAX,
        DUMB_AGENT_TYPICAL_VOL_MIN,NORMAL_PROCESSING_LATENCY,
    },
};
use crate::{
    agents::latency::DUMB_AGENT_TICKS_UNTIL_ACTIVE,
    types::order::{Order, OrderRequest, Side, Trade},
    simulation::orchestra::ShadowBookHandle,
};

// The struct is now a container of shareable handles.
// Deriving Clone is cheap as it only clones the Arcs.
#[derive(Clone)]
pub struct DumbAgent {
    id: usize,
    // Senders are Clone, so no wrapping needed.
    order_channel : Sender<OrderRequest>,
    // Receivers are not Clone, so we wrap them to be able to move handles into threads.
    ack_channel: Arc<Mutex<Receiver<Order>>>,
    port_channel: Arc<Mutex<Receiver<Trade>>>,
    // This is already an Arc<RwLock<...>>.
    view_handle: ShadowBookHandle,
    // All mutable state is wrapped for thread-safe interior mutability.
    inventory: Arc<RwLock<HashMap<u64, i64>>>,
    ticks_until_active: Arc<Mutex<u32>>,
    open_orders: Arc<RwLock<HashMap<u64, Order>>>,
    cash: Arc<RwLock<f64>>,
    margin: Arc<RwLock<f64>>,
    port_value: Arc<RwLock<f64>>,
}

impl DumbAgent {
    pub fn new(id: usize,tx_order:Sender<OrderRequest>,rx_order:Receiver<Order>,pt_order:Receiver<Trade>,view:ShadowBookHandle) -> Self {
        Self {
            id,
            order_channel: tx_order,
            // Wrap non-cloneable items in Arc<Mutex<T>>
            ack_channel: Arc::new(Mutex::new(rx_order)),
            port_channel: Arc::new(Mutex::new(pt_order)),
            view_handle: view,
            // Wrap state in Arc<RwLock<T>> or Arc<Mutex<T>>
            inventory: Arc::new(RwLock::new(HashMap::new())),
            ticks_until_active: Arc::new(Mutex::new(DUMB_AGENT_TICKS_UNTIL_ACTIVE)),
            open_orders: Arc::new(RwLock::new(HashMap::new())),
            cash: Arc::new(RwLock::new(1_000_000_000.0)),
            margin: Arc::new(RwLock::new(4_000_000_000.0)),
            port_value: Arc::new(RwLock::new(0.0)),
        }
    }

    // --- INTERNAL WORKER FUNCTIONS ---
    // These functions contain the actual logic and are designed to be called from any context
    // (e.g., a thread or a public API method) by passing the required state handles.

    /// The blocking, long-running logic for the portfolio update thread.
    fn run_portfolio_updater_internal(
        port_rx: &Receiver<Trade>,
        inventory: &Arc<RwLock<HashMap<u64, i64>>>,
        cash: &Arc<RwLock<f64>>,
        open_orders: &Arc<RwLock<HashMap<u64, Order>>>,
        agent_id: usize,
    ) {
        // This loop will block here until a trade arrives or the channel closes.
        while let Ok(tr) = port_rx.recv() {
            // Only process trades relevant to this agent.
            if tr.maker_agent_id == agent_id || tr.taker_agent_id == agent_id {
                // Lock the specific state needed for the update.
                let mut inventory_lock = inventory.write().unwrap();
                let mut cash_lock = cash.write().unwrap();
                let mut open_orders_lock = open_orders.write().unwrap();
                
                // update the inventory for the specific stock_id
                let stock_id = tr.stock_id;
                let vol = tr.volume as i64 * if tr.taker_side == Side::Buy { 1 } else { -1 };
                *inventory_lock.entry(stock_id).or_insert(0) += vol;
    
                // update cash based on the trade price and volume
                *cash_lock -= (tr.price as f64 / 100.0) * tr.volume as f64;
                
                // update the open orders
                if tr.maker_agent_id == agent_id {
                    if let Some(o) = open_orders_lock.get_mut(&tr.maker_order_id) {
                        o.filled += tr.volume;
                        if o.filled >= o.volume {
                            open_orders_lock.remove(&tr.maker_order_id);
                        }
                    }
                }
            }
            // All locks are released here at the end of the scope.
        }
    }

    /// The blocking, long-running logic for the acknowledgement listener thread.
    fn run_ack_listener_internal(
        ack_rx: &Receiver<Order>,
        open_orders: &Arc<RwLock<HashMap<u64, Order>>>,
    ) {
        while let Ok(order) = ack_rx.recv() {
            let mut open_orders_lock = open_orders.write().unwrap();
            open_orders_lock.insert(order.id, order);
        }
    }

    /// The logic for a single decision tick.
    fn decide_actions_internal(
        id: usize,
        ticks_until_active: &Arc<Mutex<u32>>,
        cash: &Arc<RwLock<f64>>,
        margin: &Arc<RwLock<f64>>,
        view_handle: &ShadowBookHandle,
        order_channel: &Sender<OrderRequest>,
    ) {
        // Lock ticks, decrement, and check if active. Drop lock immediately.
        {
            let mut ticks = ticks_until_active.lock().unwrap();
            if *ticks > 0 {
                *ticks -= 1;
                return;
            }
        } // Lock on `ticks_until_active` is released here.

        let view = view_handle.read().unwrap();
        let mut rng = rand::thread_rng();

        let universe: Vec<u64> = view.stocks.get_all_ids();
        if universe.is_empty() {
            return;
        }
        //let stock_id = *universe.choose(&mut rng).unwrap();
        for stock_id in universe{
        for _ in 0..DUMB_AGENT_NUM_TRADERS {
            if rng.gen_bool(DUMB_AGENT_ACTION_PROB) {
                let side = if rng.gen_bool(0.5) { Side::Buy } else { Side::Sell };
                let volume = if rng.gen_bool(DUMB_AGENT_LARGE_VOL_CHANCE) {
                    rng.gen_range(DUMB_AGENT_LARGE_VOL_MIN..=DUMB_AGENT_LARGE_VOL_MAX)
                } else {
                    rng.gen_range(DUMB_AGENT_TYPICAL_VOL_MIN..=DUMB_AGENT_TYPICAL_VOL_MAX)
                };

                if side == Side::Buy {
                    if let Some(px) = view.get_mid_price(stock_id) {
                        let cost = volume as f64 * (px as f64 / 100.0);
                        // Lock cash and margin for reading, drop locks immediately.
                        let current_cash = *cash.read().unwrap();
                        let current_margin = *margin.read().unwrap();
                        if cost > current_cash + current_margin {
                            continue;
                        }
                    }
                }

                let order_req = OrderRequest::MarketOrder {
                    agent_id: id,
                    stock_id,
                    side,
                    volume,
                };
                //std::thread::sleep(std::time::Duration::from_millis(NORMAL_PROCESSING_LATENCY as u64));
                order_channel.send(order_req).expect("Failed to send order request");
            }
        }}
    }
}

// -----------------------------------------------------------------------------
//  Agent impl
// -----------------------------------------------------------------------------
impl Agent for DumbAgent {
    fn run(&mut self) {
        // --- 1. Clone all necessary handles for the new threads ---
        let portfolio_rx_handle = self.port_channel.clone();
        let ack_rx_handle = self.ack_channel.clone();
        let inventory_handle = self.inventory.clone();
        let cash_handle = self.cash.clone();
        let open_orders_handle_1 = self.open_orders.clone();
        let open_orders_handle_2 = self.open_orders.clone();
        let agent_id = self.id;

        // --- 2. Spawn threads that call the INTERNAL worker functions ---
        thread::spawn(move || {
            // This thread takes ownership of the receiver from the Mutex.
            let rx = portfolio_rx_handle.lock().unwrap();
            // The thread's entire job is to call our dedicated, blocking function.
            Self::run_portfolio_updater_internal(&rx, &inventory_handle, &cash_handle, &open_orders_handle_1, agent_id);
        });

        thread::spawn(move || {
            let rx = ack_rx_handle.lock().unwrap();
            // The logic lives in the dedicated function, not in this closure.
            Self::run_ack_listener_internal(&rx, &open_orders_handle_2);
        });

        // --- 3. The main thread runs the decision loop ---
        loop {
            self.decide_actions();
            thread::sleep(std::time::Duration::from_micros(20));
        }
    }

    fn decide_actions(&mut self) {
        // The public API method is a thin wrapper that gathers dependencies and calls the worker.
        Self::decide_actions_internal(
            self.id,
            &self.ticks_until_active,
            &self.cash,
            &self.margin,
            &self.view_handle,
            &self.order_channel,
        );
    }

    fn buy_stock(&mut self, stock_id: u64, volume: u64) {
        let order_req = OrderRequest::MarketOrder {
            agent_id: self.id,
            stock_id,
            side: Side::Buy,
            volume,
        };
        self.order_channel.send(order_req).expect("Failed to send order request");
    }

    fn sell_stock(&mut self, stock_id: u64, volume: u64) {
        let order_req = OrderRequest::MarketOrder {
            agent_id: self.id,
            stock_id,
            side: Side::Sell,
            volume,
        };
        self.order_channel.send(order_req).expect("Failed to send order request");
    }

    fn margin_call(&mut self) {
        let current_cash = *self.cash.read().unwrap();
        let current_margin = *self.margin.read().unwrap();

        if current_cash < -current_margin {
            let inventory_lock = self.inventory.read().unwrap();
            for (&stock_id, &vol) in inventory_lock.iter() {
                if vol > 0 {
                    let order_req = OrderRequest::MarketOrder {
                        agent_id: self.id,
                        stock_id,
                        side: Side::Sell,
                        volume: vol.unsigned_abs(),
                    };
                    self.order_channel.send(order_req).expect("Failed to send liquidation order");
                }
            }
        }
    }

    /* ---------- bookkeeping ---------- */
    // These methods now serve as non-blocking pollers for any pending messages.

    fn acknowledge_order(&mut self) {
        // Lock the mutex to get access to the receiver, then try_recv.
        let rx = self.ack_channel.lock().unwrap();
        while let Ok(order) = rx.try_recv() {
            let mut open_orders_lock = self.open_orders.write().unwrap();
            open_orders_lock.insert(order.id, order);
        }
    }

    fn update_portfolio(&mut self) {
        let rx = self.port_channel.lock().unwrap();
        while let Ok(tr) = rx.try_recv() {
            // This logic is duplicated from the internal worker.
            // In a larger system, you might factor this into a fourth function
            // to avoid duplication, but for this example, it's explicit.
            if tr.maker_agent_id == self.id || tr.taker_agent_id == self.id {
                let mut inventory_lock = self.inventory.write().unwrap();
                let mut cash_lock = self.cash.write().unwrap();
                let mut open_orders_lock = self.open_orders.write().unwrap();
                
                let stock_id = tr.stock_id;
                let vol = tr.volume as i64 * if tr.taker_side == Side::Buy { 1 } else { -1 };
                *inventory_lock.entry(stock_id).or_insert(0) += vol;
                *cash_lock -= (tr.price as f64 / 100.0) * tr.volume as f64;
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

    fn cancel_open_order(&mut self, _id: u64) {
        //vec![] // not implemented
    }

    /* ---------- misc ---------- */

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
                acc + vol as f64 * (px as f64 / 100.0)
            } else {
                acc
            }
        });
        
        // Lock and update the shared port_value state.
        *self.port_value.write().unwrap() = new_port_value;
        new_port_value
    }
}