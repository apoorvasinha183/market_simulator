// src/agents/ipo_agent.rs
use crossbeam_channel::{Receiver, Sender};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;

use super::agent_trait::{Agent, MarketView};
use crate::{
    simulation::orchestra::ShadowBookHandle,
    types::order::{Order, OrderRequest, Side, Trade},
};

/// IPO agent: posts one ladder of sell limits at boot, then passively listens for fills.
#[derive(Clone)]
pub struct IpoAgent {
    id: usize,
    // Communication and View Handles
    order_channel: Sender<OrderRequest>,
    ack_channel: Arc<Mutex<Receiver<Order>>>,
    port_channel: Arc<Mutex<Receiver<Trade>>>,
    view_handle: ShadowBookHandle,
    // State Handles
    inventory: Arc<RwLock<HashMap<u64, i64>>>,
    has_acted: Arc<Mutex<bool>>,
    open_orders: Arc<RwLock<HashMap<u64, Order>>>,
    cash: Arc<RwLock<f64>>,
    port_value: Arc<RwLock<f64>>,
}

impl IpoAgent {
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
            // The agent starts with no inventory. It is the source of the IPO shares.
            // Its inventory will become negative as it sells.
            inventory: Arc::new(RwLock::new(HashMap::new())),
            has_acted: Arc::new(Mutex::new(false)),
            open_orders: Arc::new(RwLock::new(HashMap::new())),
            cash: Arc::new(RwLock::new(0.0)),
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
            if tr.maker_agent_id == agent_id {
                // IPO agent is always a maker
                let mut inventory_lock = inventory.write().unwrap();
                let mut cash_lock = cash.write().unwrap();
                let mut open_orders_lock = open_orders.write().unwrap();

                // Agent sells, so inventory decreases and cash increases.
                let vol_delta = -(tr.volume as i64);
                *inventory_lock.entry(tr.stock_id).or_insert(0) += vol_delta;
                *cash_lock -= vol_delta as f64 * (tr.price as f64 / 100.0);

                if let Some(o) = open_orders_lock.get_mut(&tr.maker_order_id) {
                    o.filled += tr.volume;
                    if o.filled >= o.volume {
                        open_orders_lock.remove(&tr.maker_order_id);
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

    /// The one-time action of placing the IPO sell ladder.
    fn decide_initial_actions_internal(
        id: usize,
        has_acted: &Arc<Mutex<bool>>,
        view_handle: &ShadowBookHandle,
        order_channel: &Sender<OrderRequest>,
    ) {
        // Use a lock to ensure this block only ever runs once, even if called multiple times.
        let mut acted_lock = has_acted.lock().unwrap();
        if *acted_lock {
            return;
        }
        *acted_lock = true;
        // Lock is released when `acted_lock` goes out of scope.

        let view = view_handle.read().unwrap();

        let stock_id = match view.stocks.get_all_ids().first() {
            Some(id) => *id,
            None => return, // no universe? can't act.
        };

        // This agent introduces 1,000,000 shares into the market.
        let total_ipo_shares = 1_000_000;
        let num_levels = 20;
        let vol_per = (total_ipo_shares / num_levels) as u64;
        let start_px: u64 = 15_000; // $150.00
        let tick: u64 = 5; // $0.05

        println!(
            "[IpoAgent {}] Placing IPO ladder for stock {}",
            id, stock_id
        );
        for i in 0..num_levels {
            let order_req = OrderRequest::LimitOrder {
                agent_id: id,
                stock_id,
                side: Side::Sell,
                price: start_px + (i as u64) * tick,
                volume: vol_per,
            };
            order_channel
                .send(order_req)
                .expect("Failed to send IPO order");
        }
    }
}

// -----------------------------------------------------------------------------
//  Agent impl
// -----------------------------------------------------------------------------
impl Agent for IpoAgent {
    fn run(&mut self) {
        // --- Spawn the listener threads. They need to run for the whole simulation. ---
        let portfolio_rx_handle = self.port_channel.clone();
        let ack_rx_handle = self.ack_channel.clone();
        let inventory_handle = self.inventory.clone();
        let cash_handle = self.cash.clone();
        let open_orders_handle = self.open_orders.clone();
        let agent_id = self.id;
        //let open_orders_handle_for_portfolio = self.open_orders.clone();
        let open_orders_handle_for_acks = self.open_orders.clone();

        thread::spawn(move || {
            let rx = portfolio_rx_handle.lock().unwrap();
            Self::run_portfolio_updater_internal(
                &rx,
                &inventory_handle,
                &cash_handle,
                &open_orders_handle,
                agent_id,
            );
        });

        thread::spawn(move || {
            let rx = ack_rx_handle.lock().unwrap();
            Self::run_ack_listener_internal(&rx, &open_orders_handle_for_acks);
        });

        // --- Perform the one-time action ---
        // A small sleep gives the simulation a moment to stabilize before the IPO dump.
        thread::sleep(std::time::Duration::from_millis(50));
        self.decide_actions();

        println!(
            "[IpoAgent {}] IPO orders placed. Now in passive listening mode.",
            self.id
        );

        // --- Go into a passive sleep loop ---
        // The agent's main thread has no more decisions to make, but it must stay alive
        // to keep the listener threads running.
        loop {
            thread::sleep(std::time::Duration::from_secs(10));
        }
    }

    fn decide_actions(&mut self) {
        Self::decide_initial_actions_internal(
            self.id,
            &self.has_acted,
            &self.view_handle,
            &self.order_channel,
        );
    }

    // This agent does not perform these actions after the initial IPO.
    fn buy_stock(&mut self, _id: u64, _v: u64) {}
    fn sell_stock(&mut self, _id: u64, _v: u64) {}
    fn margin_call(&mut self) {}

    /* bookkeeping ---------------------------------------------------------- */
    fn acknowledge_order(&mut self) {
        let rx = self.ack_channel.lock().unwrap();
        while let Ok(order) = rx.try_recv() {
            self.open_orders.write().unwrap().insert(order.id, order);
        }
    }

    fn update_portfolio(&mut self) {
        let rx = self.port_channel.lock().unwrap();
        while let Ok(tr) = rx.try_recv() {
            if tr.maker_agent_id == self.id {
                let mut inventory_lock = self.inventory.write().unwrap();
                let mut cash_lock = self.cash.write().unwrap();
                let mut open_orders_lock = self.open_orders.write().unwrap();
                let vol_delta = -(tr.volume as i64);
                *inventory_lock.entry(tr.stock_id).or_insert(0) += vol_delta;
                *cash_lock -= vol_delta as f64 * (tr.price as f64 / 100.0);
                if let Some(o) = open_orders_lock.get_mut(&tr.maker_order_id) {
                    o.filled += tr.volume;
                    if o.filled >= o.volume {
                        open_orders_lock.remove(&tr.maker_order_id);
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

    /* misc getters --------------------------------------------------------- */
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
