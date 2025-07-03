// src/agents/momentum_agent.rs
/*
use super::{
    agent_trait::{Agent, MarketView},
    config::{
        MOMENTUM_AGENT_ACTION_PROB, MOMENTUM_AGENT_MAX_OFFSET, MOMENTUM_AGENT_NUM_TRADERS,
        MOMENTUM_AGENT_VOL_MAX, MOMENTUM_AGENT_VOL_MIN,
    },
};
use crate::{
    simulation::orchestra::ShadowBookHandle,
    types::order::{Order, OrderRequest, Side, Trade},
};
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
        Self {
            id,
            order_channel,
            ack_channel: Arc::new(Mutex::new(ack_channel)),
            port_channel: Arc::new(Mutex::new(port_channel)),
            view_handle,
            price_history: Arc::new(RwLock::new(HashMap::new())),
            open_orders: Arc::new(RwLock::new(HashMap::new())),
            cash: Arc::new(RwLock::new(100_000.0)),
            inventory: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Agent for MomentumAgent {
    fn run(&mut self) {
        loop {
            self.decide_actions();
            thread::sleep(std::time::Duration::from_millis(100));
        }
    }

    fn decide_actions(&mut self) {
        // Implementation to follow
    }

    fn get_id(&self) -> usize {
        self.id
    }

    fn clone_agent(&self) -> Box<dyn Agent> {
        Box::new(self.clone())
    }
}

impl Clone for MomentumAgent {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            order_channel: self.order_channel.clone(),
            ack_channel: self.ack_channel.clone(),
            port_channel: self.port_channel.clone(),
            view_handle: self.view_handle.clone(),
            price_history: self.price_history.clone(),
            open_orders: self.open_orders.clone(),
            cash: self.cash.clone(),
            inventory: self.inventory.clone(),
        }
    }
}
    */
