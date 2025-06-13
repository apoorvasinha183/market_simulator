// src/simulators/order_book.rs

use crate::types::order::{Order, Side};
use std::collections::{BTreeMap, VecDeque};

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

    /// Adds a resting order to the book.
    /// This is now private as it's only called internally after processing.
    fn add_limit_order(&mut self, order: Order) {
        let book_side = match order.side {
            Side::Buy => &mut self.bids,
            Side::Sell => &mut self.asks,
        };
        let level = book_side.entry(order.price).or_default();
        // --- BUG FIX ---
        // Only add the *remaining* volume of the order to the total volume.
        let remaining_volume = order.volume.saturating_sub(order.filled);
        level.total_volume += remaining_volume;
        level.orders.push_back(order);
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

                        maker_order.filled += trade_volume;
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
        for book_side in [&mut self.bids, &mut self.asks] {
            let mut price_to_remove = None;
            let mut found_and_cancelled = false;

            if let Some((price, level)) = book_side.iter_mut().find(|(_, level)| level.orders.iter().any(|o| o.id == order_id)) {
                if let Some(index) = level.orders.iter().position(|o| o.id == order_id) {
                    if level.orders[index].agent_id == agent_id {
                        let cancelled_order = level.orders.remove(index).unwrap();
                        let remaining_volume = cancelled_order.volume.saturating_sub(cancelled_order.filled);
                        level.total_volume = level.total_volume.saturating_sub(remaining_volume);
                        found_and_cancelled = true;
                    }
                }
                // --- BUG FIX ---
                // If the level is now empty, mark its price for removal from the book.
                if level.orders.is_empty() {
                    price_to_remove = Some(*price);
                }
            }
            if let Some(price) = price_to_remove {
                book_side.remove(&price);
            }
            if found_and_cancelled {
                return true;
            }
        }
        false
    }
}

// -----------------------------------------------------------------------------
//  Unit Tests
// -----------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::order::{Order, Side};

    fn new_order(id: u64, agent_id: usize, side: Side, price: u64, volume: u64) -> Order {
        Order { id, agent_id, side, price, volume, filled: 0 }
    }

    #[test]
    fn test_add_simple_limit_order() {
        let mut book = OrderBook::new();
        let order = new_order(1, 1, Side::Buy, 100, 50);
        book.add_limit_order(order);
        let level = book.bids.get(&100).unwrap();
        assert_eq!(level.total_volume, 50);
    }

    #[test]
    fn test_market_order_simple_fill() {
        let mut book = OrderBook::new();
        book.add_limit_order(new_order(1, 1, Side::Sell, 100, 50));
        let trades = book.process_market_order(2, Side::Buy, 30);
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].volume, 30);
        let ask_level = book.asks.get(&100).unwrap();
        assert_eq!(ask_level.total_volume, 20);
        assert_eq!(ask_level.orders[0].filled, 30);
    }

    #[test]
    fn test_market_order_full_fill_and_remove() {
        let mut book = OrderBook::new();
        book.add_limit_order(new_order(1, 1, Side::Sell, 100, 50));
        book.process_market_order(3, Side::Buy, 50);
        assert!(book.asks.get(&100).is_none());
    }
    
    #[test]
    fn test_marketable_limit_order() {
        let mut book = OrderBook::new();
        book.add_limit_order(new_order(1, 1, Side::Sell, 100, 50));
        let mut aggressive_buy = new_order(2, 2, Side::Buy, 101, 30);
        book.process_limit_order(&mut aggressive_buy);
        assert_eq!(book.asks.get(&100).unwrap().total_volume, 20);
        assert!(book.bids.is_empty());
    }

    #[test]
    fn test_marketable_limit_order_partial_fill_and_rest() {
        let mut book = OrderBook::new();
        book.add_limit_order(new_order(1, 1, Side::Sell, 100, 30));
        let mut aggressive_buy = new_order(2, 2, Side::Buy, 101, 50);
        book.process_limit_order(&mut aggressive_buy);
        assert!(!book.asks.contains_key(&100));
        assert_eq!(book.bids.len(), 1);
        let bid_level = book.bids.get(&101).unwrap();
        assert_eq!(bid_level.total_volume, 20, "The new bid should have 20 remaining volume.");
    }

    #[test]
    fn test_cancel_order_simple() {
        let mut book = OrderBook::new();
        book.add_limit_order(new_order(1, 1, Side::Buy, 100, 50));
        let success = book.cancel_order(1, 1);
        assert!(success);
        assert!(book.bids.is_empty(), "The bid side of the book should be empty after cancellation.");
    }

    #[test]
    fn test_cancel_order_fails_for_wrong_owner() {
        let mut book = OrderBook::new();
        book.add_limit_order(new_order(1, 1, Side::Buy, 100, 50));
        let success = book.cancel_order(1, 2);
        assert!(!success);
        assert_eq!(book.bids.get(&100).unwrap().total_volume, 50);
    }
}
