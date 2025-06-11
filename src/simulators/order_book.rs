// src/simulators/order_book.rs

use crate::types::order::{Order, Side};
use std::collections::{BTreeMap, VecDeque};

#[derive(Debug, Clone, Copy)]
pub struct Trade {
    pub price: u64,
    pub volume: u64,
    pub taker_agent_id: usize,
    pub maker_agent_id: usize,
    pub taker_side: Side, // <-- ADD THIS FIELD
}

pub struct OrderBook {
    pub bids: BTreeMap<u64, VecDeque<Order>>,
    pub asks: BTreeMap<u64, VecDeque<Order>>,
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
        }
    }

    pub fn add_limit_order(&mut self, order: Order) {
        let book_side = match order.side {
            Side::Buy => &mut self.bids,
            Side::Sell => &mut self.asks,
        };
        book_side.entry(order.price).or_default().push_back(order);
    }
    
    pub fn process_market_order(&mut self, taker_agent_id: usize, side: Side, mut volume_to_fill: u64) -> Vec<Trade> {
        let mut trades = Vec::new();
        let book_to_match = match side {
            Side::Buy => &mut self.asks,
            Side::Sell => &mut self.bids,
        };

        let mut empty_levels = Vec::new();
        let price_levels: Vec<u64> = match side {
            Side::Buy => book_to_match.keys().cloned().collect(),
            Side::Sell => book_to_match.keys().rev().cloned().collect(),
        };

        for price in price_levels {
            if volume_to_fill == 0 { break; }

            if let Some(level_queue) = book_to_match.get_mut(&price) {
                while let Some(maker_order) = level_queue.front_mut() {
                    if volume_to_fill == 0 { break; }

                    let trade_volume = volume_to_fill.min(maker_order.volume);
                    
                    trades.push(Trade {
                        price: maker_order.price,
                        volume: trade_volume,
                        taker_agent_id,
                        maker_agent_id: maker_order.agent_id,
                        taker_side: side, // <-- ADD THIS FIELD ON TRADE CREATION
                    });

                    maker_order.volume -= trade_volume;
                    volume_to_fill -= trade_volume;

                    if maker_order.volume == 0 {
                        level_queue.pop_front();
                    }
                }
                if level_queue.is_empty() {
                    empty_levels.push(price);
                }
            }
        }
        
        for price in empty_levels {
            book_to_match.remove(&price);
        }
        
        trades
    }
    
    // Add this function to your OrderBook impl
    pub fn process_limit_order(&mut self, order: &mut Order) -> Vec<Trade> {
        let mut trades = Vec::new();
        let taker_agent_id = order.agent_id;
        let book_to_match = match order.side {
            Side::Buy => &mut self.asks,
            Side::Sell => &mut self.bids,
        };

        let mut empty_levels = Vec::new();
        let price_levels: Vec<u64> = match order.side {
            Side::Buy => book_to_match.keys().cloned().collect(),
            Side::Sell => book_to_match.keys().rev().cloned().collect(),
        };

        for price in price_levels {
            if order.volume == 0 { break; }
            let price_is_good = match order.side {
                Side::Buy => price <= order.price,
                Side::Sell => price >= order.price,
            };
            if !price_is_good { break; }

            if let Some(level_queue) = book_to_match.get_mut(&price) {
                while let Some(maker_order) = level_queue.front_mut() {
                    if order.volume == 0 { break; }
                    let trade_volume = order.volume.min(maker_order.volume);
                    
                    trades.push(Trade {
                        price: maker_order.price,
                        volume: trade_volume,
                        taker_agent_id,
                        maker_agent_id: maker_order.agent_id,
                        taker_side: order.side, // <-- ADD THIS FIELD ON TRADE CREATION
                    });

                    maker_order.volume -= trade_volume;
                    order.volume -= trade_volume;

                    if maker_order.volume == 0 {
                        level_queue.pop_front();
                    }
                }
                if level_queue.is_empty() {
                    empty_levels.push(price);
                }
            }
        }
        
        for price in empty_levels {
            book_to_match.remove(&price);
        }
        
        if order.volume > 0 {
            self.add_limit_order(*order);
        }

        trades
    }
}