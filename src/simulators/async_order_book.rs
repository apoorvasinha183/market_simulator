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
            while let Ok(req) = self.order_rx.recv() {
                let trades = self.process_request(req);
                for trade in trades {
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
                agent_id,
                stock_id,
                side,
                price,
                volume,
            } => {
                let mut order = crate::types::Order {
                    id: 0, // The async book doesn't know the global ID, market will assign
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
                agent_id,
                stock_id: _,
                side,
                volume,
            } => self.order_book.process_market_order(agent_id, side, volume),
            OrderRequest::CancelOrder { agent_id, order_id } => {
                self.order_book.cancel_order(order_id, agent_id);
                Vec::new() // No trades from a cancel
            }
        }
    }
}