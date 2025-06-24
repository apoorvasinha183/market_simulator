// src/agents/dumb_agent.rs
use rand::{Rng, seq::SliceRandom};
use std::collections::HashMap;
use crossbeam_channel::{unbounded, Receiver, Sender};
use super::{
    agent_trait::{Agent, MarketView},
    config::{
        DUMB_AGENT_ACTION_PROB, DUMB_AGENT_LARGE_VOL_CHANCE, DUMB_AGENT_LARGE_VOL_MAX,
        DUMB_AGENT_LARGE_VOL_MIN, DUMB_AGENT_NUM_TRADERS, DUMB_AGENT_TYPICAL_VOL_MAX,
        DUMB_AGENT_TYPICAL_VOL_MIN,
    },
};
use crate::{
    agents::latency::DUMB_AGENT_TICKS_UNTIL_ACTIVE,
    types::order::{self, Order, OrderRequest, Side, Trade},
};
//allow cloning
#[derive(Debug, Clone)]
pub struct DumbAgent {
    id: usize,
    // tx channel to send order to the market
    order_channel : Sender<OrderRequest>,
    //rx channels to recieve acknowledgements and trade updates
    ack_channel: Receiver<Order>,
    // channel to update the agent's inventory
    port_channel: Receiver<Trade>,// This is redundant but I will get rid of it later. Ack channel will serve this purpose
    // update inventory as a hashmap linking the stock id to the number of shares held .(Signed so I can short)
    inventory: HashMap<u64, i64>,
    ticks_until_active: u32,
    open_orders: HashMap<u64, Order>,
    cash: f64,
    margin: f64,
    port_value: f64,
}

impl DumbAgent {
    pub fn new(id: usize,tx_order:Sender<OrderRequest>,rx_order:Receiver<Order>,pt_order:Receiver<Trade>) -> Self {
        Self {
            id,
            order_channel: tx_order,
            ack_channel: rx_order,
            port_channel: pt_order,
            // empty inventory hashmap
            inventory: HashMap::new(),
            ticks_until_active: DUMB_AGENT_TICKS_UNTIL_ACTIVE,
            open_orders: HashMap::new(),
            cash: 1_000_000_000.0,
            margin: 4_000_000_000.0,
            port_value: 0.0,
        }
    }
}

// -----------------------------------------------------------------------------
//  Agent impl
// -----------------------------------------------------------------------------
impl Agent for DumbAgent {
    fn decide_actions(&mut self, view: &MarketView)  {
        if self.ticks_until_active > 0 {
            self.ticks_until_active -= 1;
            return ;
        }

        let mut rng = rand::thread_rng();
        //let mut out = Vec::new();

        /* --- choose a random instrument for this tick --- */
        let universe: Vec<u64> = view.stocks.get_all_ids();
        if universe.is_empty() {
            return ;
        }
        let stock_id = *universe.choose(&mut rng).unwrap();

        for _ in 0..DUMB_AGENT_NUM_TRADERS {
            if rng.gen_bool(DUMB_AGENT_ACTION_PROB) {
                let side = if rng.gen_bool(0.5) {
                    Side::Buy
                } else {
                    Side::Sell
                };

                let volume = if rng.gen_bool(DUMB_AGENT_LARGE_VOL_CHANCE) {
                    rng.gen_range(DUMB_AGENT_LARGE_VOL_MIN..=DUMB_AGENT_LARGE_VOL_MAX)
                } else {
                    rng.gen_range(DUMB_AGENT_TYPICAL_VOL_MIN..=DUMB_AGENT_TYPICAL_VOL_MAX)
                };

                /* --- buying-power check --- */
                if side == Side::Buy {
                    if let Some(px) = view.get_mid_price(stock_id) {
                        let cost = volume as f64 * (px as f64 / 100.0);
                        if cost > self.cash + self.margin {
                            continue; // skip action
                        }
                    }
                }
                // this is from a prehistoric commit , do not laugh
                let reqs = if side == Side::Buy {
                    let order_req = OrderRequest::MarketOrder {
                        agent_id: self.id,
                        stock_id,
                        side,
                        volume,
                    };
                    self.order_channel.send(order_req).expect("Failed to send order request");
                } else {
                    let order_req = OrderRequest::MarketOrder {
                        agent_id: self.id,
                        stock_id,
                        side,
                        volume,
                    };
                    self.order_channel.send(order_req).expect("Failed to send order request");
                };
                //out.extend(reqs);
            }
        }
        //out
    }
    fn run(&mut self) { // loop decide actions here 
        //run in a loop
        loop{
            //spwan three threads 1. to decide actions 2. to listen to order acknowledgements 3. to listen to trade updates
            
        }
    }
    fn buy_stock(&mut self, stock_id: u64, volume: u64)  {
        // create a market order request
        let order_req = OrderRequest::MarketOrder {
            agent_id: self.id,
            stock_id,
            side: Side::Buy,
            volume,
        };
        //send the order request to the market
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

    fn margin_call(&mut self)  {
        if self.cash < -self.margin {
            // CREATE AN empty vector to hold the liquidation orders
            //let mut liquidation_orders = Vec::new();
            // sweep the inventory hashmap and burn all shares into the lqiuidation orders
            for (&stock_id, &vol) in &self.inventory {
                if vol > 0 {
                    let order_req = OrderRequest::MarketOrder {
                        agent_id: self.id,
                        stock_id,
                        side: Side::Sell,
                        volume: vol.unsigned_abs() as u64, // sell all shares
                    };
                    self.order_channel.send(order_req).expect("Failed to send liquidation order");
                }
            }
            // if the cash still doesn't cover the margin, then declare Chapter 11 bankruptcy  :)
            // clear the inventory -- Nah wait for acks to do this .
            //self.inventory.clear();
            // return the liquidation orders
            
        }

        //vec![]
    }

    /* ---------- bookkeeping ---------- */

    fn acknowledge_order(&mut self) {
        // listen to the order acknowledgements and update the open orders hashmap
        while let Ok(order) = self.ack_channel.try_recv() {
            self.open_orders.insert(order.id, order);
        }
    }

    fn update_portfolio(&mut self) {
        //extract the stock_id from the trade channel
        while let Ok(tr) = self.port_channel.try_recv() {
            // update the inventory for the specific stock_id
            let stock_id = tr.stock_id;
            let vol = tr.volume as i64 * if tr.taker_side == Side::Buy { 1 } else { -1 };
            *self.inventory.entry(stock_id).or_insert(0) += vol;

            // update cash based on the trade price and volume
            self.cash -= (tr.price as f64 / 100.0) * tr.volume as f64;
            // update the open orders since the architecture enforces that the agent making the order is this one
            if tr.maker_agent_id == self.id {
                if let Some(o) = self.open_orders.get_mut(&tr.maker_order_id) {
                    o.filled += tr.volume;
                    if o.filled >= o.volume {
                        self.open_orders.remove(&tr.maker_order_id);    }
            
        }} 
        //let trade = self.port_channel.try_recv();
        // Update the inventory for the specific stock_id
        
    }}

    fn get_pending_orders(&self) -> Vec<Order> {
        self.open_orders.values().cloned().collect()
    }

    fn cancel_open_order(&mut self, _id: u64)  {
        //vec![] // not implemented
    }

    /* ---------- misc ---------- */

    fn get_id(&self) -> usize {
        self.id
    }
    fn get_inventory(&self) -> i64 {
        // count the total inventory across all stocks
        self.inventory.values().sum()
        //self.inventory
    }
    fn clone_agent(&self) -> Box<dyn Agent> {
        Box::new(self.clone()) // clone the agent while preserving the inventory and stuff.
    }

    fn evaluate_port(&mut self, view: &MarketView) -> f64 {
        // iterate over all stocks in the inventory and calculate the total value
        // take out all the stock and use their mid price
        self.port_value = self.inventory.iter().fold(0.0, |acc, (stock_id, &vol)| {
            if let Some(px) = view.get_mid_price(*stock_id) {
                acc + vol as f64 * (px as f64 / 100.0)
            } else {
                acc
            }
        });
        self.port_value
    }
}
