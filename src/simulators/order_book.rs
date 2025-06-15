// src/simulators/order_book.rs

use crate::types::order::{Order, Side, Trade};
use std::collections::{BTreeMap, VecDeque};

pub struct OrderBook {
    bids: BTreeMap<u64, Vec<u64>>, // Price -> Vec<OrderID>
    asks: BTreeMap<u64, Vec<u64>>, // Price -> Vec<OrderID>
    orders: BTreeMap<u64, Order>,
    trades: VecDeque<Trade>,
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            orders: BTreeMap::new(),
            trades: VecDeque::with_capacity(100), // Store last 100 trades
        }
    }

    pub fn process_limit_order(&mut self, order: &mut Order) -> Vec<Trade> {
        let mut trades = Vec::new();
        let (side_to_match, book_to_match) = if order.side == Side::Buy {
            (Side::Sell, &mut self.asks)
        } else {
            (Side::Buy, &mut self.bids)
        };

        let mut potential_matches: Vec<u64> = book_to_match.keys().cloned().collect();
        if order.side == Side::Buy {
            potential_matches.sort();
        } else {
            potential_matches.sort_by(|a, b| b.cmp(a));
        }

        for price_level in potential_matches {
            if (order.side == Side::Buy && order.price < price_level)
                || (order.side == Side::Sell && order.price > price_level)
            {
                break;
            }

            if let Some(orders_at_price) = book_to_match.get_mut(&price_level) {
                let mut orders_to_remove = vec![];
                for (i, maker_order_id) in orders_at_price.iter().enumerate() {
                    if let Some(maker_order) = self.orders.get_mut(maker_order_id) {
                        let trade_volume = std::cmp::min(order.volume - order.filled, maker_order.volume - maker_order.filled);
                        if trade_volume > 0 {
                            order.filled += trade_volume;
                            maker_order.filled += trade_volume;

                            let trade = Trade {
                                symbol: order.symbol.clone(),
                                taker_agent_id: order.agent_id,
                                maker_agent_id: maker_order.agent_id,
                                taker_side: order.side,
                                price: maker_order.price,
                                volume: trade_volume,
                                taker_order_id: order.id,
                                maker_order_id: maker_order.id,
                            };
                            trades.push(trade.clone());
                            if self.trades.len() >= 100 {
                                self.trades.pop_front();
                            }
                            self.trades.push_back(trade);

                            if maker_order.filled == maker_order.volume {
                                orders_to_remove.push(i);
                                self.orders.remove(maker_order_id);
                            }
                        }
                    }
                    if order.filled == order.volume {
                        break;
                    }
                }
                for i in orders_to_remove.into_iter().rev() {
                    orders_at_price.remove(i);
                }

                if orders_at_price.is_empty() {
                    book_to_match.remove(&price_level);
                }
            }
            if order.filled == order.volume {
                self.orders.insert(order.id, order.clone());
                break;
            }
        }

        if order.filled < order.volume {
            let book_to_add = if order.side == Side::Buy { &mut self.bids } else { &mut self.asks };
            book_to_add.entry(order.price).or_default().push(order.id);
            self.orders.insert(order.id, order.clone());
        }

        trades
    }

    pub fn process_market_order(&mut self, agent_id: usize, side: Side, volume: u64, symbol: &str) -> Vec<Trade> {
        let mut order = Order {
            id: u64::MAX, // A bit of a hack for market orders
            agent_id,
            symbol: symbol.to_string(),
            side,
            price: if side == Side::Buy { u64::MAX } else { 0 },
            volume,
            filled: 0,
        };
        self.process_limit_order(&mut order)
    }

    pub fn cancel_order(&mut self, order_id: u64, agent_id: usize) {
        if let Some(order_to_cancel) = self.orders.get(&order_id) {
            if order_to_cancel.agent_id == agent_id {
                let book = if order_to_cancel.side == Side::Buy {
                    &mut self.bids
                } else {
                    &mut self.asks
                };
                if let Some(orders_at_price) = book.get_mut(&order_to_cancel.price) {
                    orders_at_price.retain(|&id| id != order_id);
                    if orders_at_price.is_empty() {
                        book.remove(&order_to_cancel.price);
                    }
                }
                self.orders.remove(&order_id);
            }
        }
    }

    // FIXED: Added missing `get_bids` method
    pub fn get_bids(&self) -> &BTreeMap<u64, Vec<u64>> {
        &self.bids
    }

    // FIXED: Added missing `get_asks` method
    pub fn get_asks(&self) -> &BTreeMap<u64, Vec<u64>> {
        &self.asks
    }

    // FIXED: Added missing `get_trades` method
    pub fn get_trades(&self) -> &VecDeque<Trade> {
        &self.trades
    }
    
    // FIXED: Added missing `get_depth` method
    pub fn get_depth(&self) -> (Vec<(u64, u64)>, Vec<(u64, u64)>) {
        let bids_depth = self.bids.iter().map(|(price, orders)| {
            let volume = orders.iter().map(|id| self.orders.get(id).map_or(0, |o| o.volume - o.filled)).sum();
            (*price, volume)
        }).collect();
        let asks_depth = self.asks.iter().map(|(price, orders)| {
            let volume = orders.iter().map(|id| self.orders.get(id).map_or(0, |o| o.volume - o.filled)).sum();
            (*price, volume)
        }).collect();

        (bids_depth, asks_depth)
    }
}