// src/simulators/order_book.rs

use crate::types::order::{Order, Side};
use std::collections::{BTreeMap, VecDeque};
#[derive(Debug, Clone, Copy)]



pub struct Trade {



    pub price: u64,



    pub volume: u64,



    pub taker_agent_id: usize,



    pub maker_agent_id: usize,



    pub taker_side: Side,



    pub maker_order_id: u64



}
// NEW: A struct to hold the aggregate state of a single price level.
#[derive(Debug, Default)]
pub struct PriceLevel {
    pub total_volume: u64,       // For the visualizer to read directly.
    pub orders: VecDeque<Order>, // The FIFO queue of orders for the matching engine.
}

pub struct OrderBook {
    // The book now maps a price to its stateful PriceLevel.
    pub bids: BTreeMap<u64, PriceLevel>,
    pub asks: BTreeMap<u64, PriceLevel>,
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
        }
    }

    fn add_limit_order(&mut self, order: Order) {
        let book_side = match order.side {
            Side::Buy => &mut self.bids,
            Side::Sell => &mut self.asks,
        };
        let level = book_side.entry(order.price).or_default();
        level.total_volume += order.volume; // Update aggregate volume
        level.orders.push_back(order);    // Add the specific order
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

            if let Some(level) = book_to_match.get_mut(&price) {
                while let Some(maker_order) = level.orders.front_mut() {
                    if volume_to_fill == 0 { break; }

                    let remaining_volume = maker_order.volume.saturating_sub(maker_order.filled);
                    if remaining_volume == 0 {
                        level.orders.pop_front();
                        continue;
                    }

                    let trade_volume = volume_to_fill.min(remaining_volume);
                    
                    if trade_volume > 0 {
                        trades.push(Trade {
                            price: maker_order.price,
                            volume: trade_volume,
                            taker_agent_id,
                            maker_agent_id: maker_order.agent_id,
                            maker_order_id: maker_order.id,
                            taker_side: side,
                        });

                        // --- THIS IS THE FUCKING FIX ---
                        // Update the individual order's state
                        maker_order.filled += trade_volume;
                        // AND update the aggregate volume for the whole price level
                        level.total_volume -= trade_volume;
                        
                        volume_to_fill -= trade_volume;
                    }
                    
                    if maker_order.filled >= maker_order.volume {
                        level.orders.pop_front();
                    }
                }
                if level.orders.is_empty() {
                    empty_levels.push(price);
                }
            }
        }
        
        for price in empty_levels {
            book_to_match.remove(&price);
        }
        
        trades
    }
    
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
            let order_remaining = order.volume.saturating_sub(order.filled);
            if order_remaining == 0 { break; }

            let price_is_good = match order.side {
                Side::Buy => price <= order.price,
                Side::Sell => price >= order.price,
            };
            if !price_is_good { break; }

            if let Some(level) = book_to_match.get_mut(&price) {
                while let Some(maker_order) = level.orders.front_mut() {
                    let order_remaining = order.volume.saturating_sub(order.filled);
                    if order_remaining == 0 { break; }

                    let maker_remaining = maker_order.volume.saturating_sub(maker_order.filled);
                    let trade_volume = order_remaining.min(maker_remaining);
                    
                    if trade_volume > 0 {
                        trades.push(Trade {
                            price: maker_order.price,
                            volume: trade_volume,
                            taker_agent_id,
                            maker_agent_id: maker_order.agent_id,
                            maker_order_id: maker_order.id,
                            taker_side: order.side,
                        });

                        // --- THIS IS THE FUCKING FIX ---
                        maker_order.filled += trade_volume;
                        order.filled += trade_volume;
                        level.total_volume -= trade_volume;
                    }

                    if maker_order.filled >= maker_order.volume {
                        level.orders.pop_front();
                    }
                }
                if level.orders.is_empty() {
                    empty_levels.push(price);
                }
            }
        }
        
        for price in empty_levels {
            book_to_match.remove(&price);
        }
        
        if order.filled < order.volume {
            self.add_limit_order(*order);
        }

        trades
    }
    pub fn cancel_order(&mut self, order_id: u64, agent_id: usize) -> bool {
        // We have to search both sides of the book.
        for book_side in [&mut self.bids, &mut self.asks] {
            // Find which price level the order is at.
            if let Some(level) = book_side.values_mut().find(|level| level.orders.iter().any(|o| o.id == order_id)) {
                // Now, find the specific order in that level's queue.
                if let Some(index) = level.orders.iter().position(|o| o.id == order_id) {
                    // Security check: only the owner can cancel.
                    if level.orders[index].agent_id == agent_id {
                        let cancelled_order = level.orders.remove(index).unwrap();
                        let remaining_volume = cancelled_order.volume.saturating_sub(cancelled_order.filled);
                        level.total_volume = level.total_volume.saturating_sub(remaining_volume);
                        return true; // Success!
                    }
                }
            }
        }
        false // Order not found or not owned by the agent.
    }
}
