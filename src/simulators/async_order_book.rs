// src/simulators/async_order_book.rs

use crate::simulators::order_book::OrderBook;
use crate::types::{OrderRequest, Trade};
use crossbeam_channel::{Receiver, Sender};
use std::thread;

/// An asynchronous wrapper around the OrderBook that runs in its own thread.
pub struct AsyncOrderBook {
    order_rx: Receiver<OrderRequest>,
    trade_tx: Sender<Trade>,
    order_book: OrderBook,
}

impl AsyncOrderBook {
    /// Creates a new AsyncOrderBook and the channels for communication.
    /// Returns the sender for orders and the receiver for trades.
    pub fn new() -> (Sender<OrderRequest>, Receiver<Trade>) {
        let (order_tx, order_rx) = crossbeam_channel::unbounded();
        let (trade_tx, trade_rx) = crossbeam_channel::unbounded();

        let book = Self {
            order_rx,
            trade_tx,
            order_book: OrderBook::new(),
        };

        book.run();

        (order_tx, trade_rx)
    }

    /// The main loop for the order book thread.
    fn run(mut self) {
        thread::spawn(move || {
            //let mut ntrades:u64 = 0;
            while let Ok(req) = self.order_rx.recv() {
                let trades = self.process_request(req);
                for trade in trades {
                    //ntrades += 1;
                    // Log every 10000 trades upto the first 100k
                    /* 
                    if (ntrades % 10000 == 0) && (ntrades < 100000) {
                        println!("Processed {} trades", ntrades);
                    } */
                    if self.trade_tx.send(trade).is_err() {
                        // If the receiver is dropped, the market is likely shutting down.
                        break;
                    }
                }
            }
        });
    }

    /// Processes a single order request and returns any resulting trades.
    fn process_request(&mut self, req: OrderRequest) -> Vec<Trade> {
        match req {
            OrderRequest::LimitOrder {
                order_id, // Capture the order_id
                agent_id,
                stock_id,
                side,
                price,
                volume,
            } => {
                let mut order = crate::types::Order {
                    id: order_id, // Use the captured order_id
                    agent_id,
                    stock_id,
                    side,
                    price,
                    volume,
                    filled: 0,
                };
                self.order_book.process_limit_order(&mut order)
            }
            OrderRequest::MarketOrder {
                order_id, // Capture the order_id
                agent_id,
                stock_id,
                side,
                volume,
            } => {
                let _order = crate::types::Order {
                    id: order_id, // Use the captured order_id
                    agent_id,
                    stock_id,
                    side,
                    volume,
                    price: 0, // Market orders don't have a price in the request, will be filled by order book
                    filled: 0,
                };
                self.order_book
                    .process_market_order(order_id, agent_id, side, volume) // Pass order_id
            }
            OrderRequest::CancelOrder { agent_id, order_id } => {
                self.order_book.cancel_order(order_id, agent_id);
                Vec::new() // No trades from a cancel
            }
        }
    }
}
