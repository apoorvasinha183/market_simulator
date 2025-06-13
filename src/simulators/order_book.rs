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
// -----------------------------------------------------------------------------
//  Unit Tests
// -----------------------------------------------------------------------------
// The #[cfg(test)] attribute tells the Rust compiler to only compile this
// module when we run `cargo test`, so it's not included in the final binary.
#[cfg(test)]
mod tests {
    use super::*; // Import everything from the parent module (OrderBook, Trade, etc.)
    use crate::types::order::{Order, Side}; // Import necessary types

    // A helper function to create a new test order with less boilerplate.
    fn new_order(id: u64, agent_id: usize, side: Side, price: u64, volume: u64) -> Order {
        Order { id, agent_id, side, price, volume, filled: 0 }
    }

    #[test]
    fn test_add_simple_limit_order() {
        // Arrange
        let mut book = OrderBook::new();
        let order = new_order(1, 1, Side::Buy, 100, 50);

        // Act
        book.add_limit_order(order);

        // Assert
        assert_eq!(book.bids.len(), 1, "A bid price level should have been created.");
        let level = book.bids.get(&100).unwrap();
        assert_eq!(level.total_volume, 50, "The total volume at the price level should be 50.");
        assert_eq!(level.orders.len(), 1, "There should be one order in the queue.");
        assert_eq!(level.orders[0].id, 1, "The order ID should match.");
    }

    #[test]
    fn test_market_order_simple_fill() {
        // Arrange
        let mut book = OrderBook::new();
        book.add_limit_order(new_order(1, 1, Side::Sell, 100, 50));

        // Act
        let trades = book.process_market_order(2, Side::Buy, 30);

        // Assert
        assert_eq!(trades.len(), 1, "There should have been exactly one trade.");
        let trade = trades[0];
        assert_eq!(trade.price, 100, "The trade price should be 100.");
        assert_eq!(trade.volume, 30, "The trade volume should be 30.");
        assert_eq!(trade.taker_agent_id, 2, "The taker ID should be 2.");
        assert_eq!(trade.maker_agent_id, 1, "The maker ID should be 1.");
        assert_eq!(trade.maker_order_id, 1, "The maker's order ID should be 1.");

        let ask_level = book.asks.get(&100).unwrap();
        assert_eq!(ask_level.total_volume, 20, "The remaining volume on the book should be 20.");
        assert_eq!(ask_level.orders[0].filled, 30, "The resting order should show 30 filled.");
    }

    #[test]
    fn test_market_order_full_fill_and_remove() {
        // Arrange
        let mut book = OrderBook::new();
        book.add_limit_order(new_order(1, 1, Side::Sell, 100, 50));
        book.add_limit_order(new_order(2, 1, Side::Sell, 101, 50));

        // Act
        let trades = book.process_market_order(3, Side::Buy, 50);

        // Assert
        assert_eq!(trades.len(), 1, "A single trade should occur.");
        // --- THIS IS THE FIX ---
        // Use the .is_none() method to check for an empty Option.
        assert!(book.asks.get(&100).is_none(), "The price level at 100 should be completely removed.");
        assert!(book.asks.contains_key(&101), "The price level at 101 should still exist.");
    }
    
    #[test]
    fn test_marketable_limit_order() {
        // Arrange
        let mut book = OrderBook::new();
        book.add_limit_order(new_order(1, 1, Side::Sell, 100, 50));
        let mut aggressive_buy = new_order(2, 2, Side::Buy, 101, 30); // Priced above the ask

        // Act
        let trades = book.process_limit_order(&mut aggressive_buy);

        // Assert
        assert_eq!(trades.len(), 1, "The marketable limit order should have executed a trade.");
        assert_eq!(trades[0].price, 100, "Trade should happen at the resting order's price.");
        assert_eq!(trades[0].volume, 30, "Trade volume should match the aggressive order's volume.");
        assert_eq!(book.asks.get(&100).unwrap().total_volume, 20, "The resting order should have 20 volume left.");
        assert!(book.bids.is_empty(), "The aggressive buy order should not rest on the book as it was fully filled.");
    }

    #[test]
    fn test_marketable_limit_order_partial_fill_and_rest() {
        // Arrange
        let mut book = OrderBook::new();
        book.add_limit_order(new_order(1, 1, Side::Sell, 100, 30));
        let mut aggressive_buy = new_order(2, 2, Side::Buy, 101, 50);

        // Act
        let trades = book.process_limit_order(&mut aggressive_buy);

        // Assert
        assert_eq!(trades.len(), 1, "There should be one trade.");
        assert_eq!(trades[0].volume, 30, "The trade should be for 30 shares.");
        assert!(!book.asks.contains_key(&100), "The ask at 100 should be completely filled and removed.");
        
        assert_eq!(book.bids.len(), 1, "The remaining volume should be placed on the bid side.");
        let bid_level = book.bids.get(&101).unwrap();
        assert_eq!(bid_level.total_volume, 20, "The new bid should have 20 remaining volume.");
        assert_eq!(bid_level.orders[0].id, 2, "The new bid should have the correct order ID.");
    }

    #[test]
    fn test_cancel_order_simple() {
        // Arrange
        let mut book = OrderBook::new();
        book.add_limit_order(new_order(1, 1, Side::Buy, 100, 50));
        
        // Act
        let success = book.cancel_order(1, 1);

        // Assert
        assert!(success, "The cancellation should have been successful.");
        assert!(book.bids.is_empty(), "The bid side of the book should be empty after cancellation.");
    }

    #[test]
    fn test_cancel_order_fails_for_wrong_owner() {
        // Arrange
        let mut book = OrderBook::new();
        book.add_limit_order(new_order(1, 1, Side::Buy, 100, 50));
        
        // Act: Agent 2 tries to cancel Agent 1's order
        let success = book.cancel_order(1, 2);

        // Assert
        assert!(!success, "The cancellation should have failed.");
        assert_eq!(book.bids.get(&100).unwrap().total_volume, 50, "The order should not have been removed.");
    }
}
