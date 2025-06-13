// src/simulators/order_book.rs

use crate::types::order::{Order, Side};
use std::collections::{BTreeMap, HashMap, VecDeque};

pub struct Trade {
    pub price: u64,
    pub volume: u64,
    pub taker_agent_id: usize,
    pub maker_agent_id: usize,
    pub taker_side: Side,
    pub maker_order_id: u64,
}

#[derive(Debug, Default)]
pub struct PriceLevel {
    pub total_volume: u64,
    pub orders: VecDeque<Order>,
}

pub struct OrderBook {
    pub bids: BTreeMap<u64, PriceLevel>,
    pub asks: BTreeMap<u64, PriceLevel>,
    order_id_map: HashMap<u64, (Side, u64)>,
}

impl OrderBook {
    pub fn new() -> Self {
        Self {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            order_id_map: HashMap::new(),
        }
    }

    fn add_limit_order(&mut self, order: Order) {
        let book_side = match order.side {
            Side::Buy => &mut self.bids,
            Side::Sell => &mut self.asks,
        };
        let level = book_side.entry(order.price).or_default();
        let remaining_volume = order.volume.saturating_sub(order.filled);
        level.total_volume += remaining_volume;
        level.orders.push_back(order);
        self.order_id_map.insert(order.id, (order.side, order.price));
    }

    pub fn process_market_order(&mut self, taker_agent_id: usize, side: Side, mut volume_to_fill: u64) -> Vec<Trade> {
        let mut trades = Vec::new();
        let mut filled_order_ids = Vec::new();
        let mut empty_levels = Vec::new();
        
        let book_to_match = match side {
            Side::Buy => &mut self.asks,
            Side::Sell => &mut self.bids,
        };

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
                        filled_order_ids.push(level.orders.pop_front().unwrap().id);
                        continue;
                    }
                    let trade_volume = volume_to_fill.min(remaining_volume);
                    trades.push(Trade { price, volume: trade_volume, taker_agent_id, maker_agent_id: maker_order.agent_id, maker_order_id: maker_order.id, taker_side: side });
                    maker_order.filled += trade_volume;
                    level.total_volume -= trade_volume;
                    volume_to_fill -= trade_volume;
                    if maker_order.filled >= maker_order.volume {
                        filled_order_ids.push(level.orders.pop_front().unwrap().id);
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
        for id in filled_order_ids {
            self.order_id_map.remove(&id);
        }
        trades
    }
    
    pub fn process_limit_order(&mut self, order: &mut Order) -> Vec<Trade> {
        let mut trades = Vec::new();
        let mut filled_order_ids = Vec::new();
        let mut empty_levels = Vec::new();
        
        let book_to_match = match order.side {
            Side::Buy => &mut self.asks,
            Side::Sell => &mut self.bids,
        };

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
                        trades.push(Trade { price, volume: trade_volume, taker_agent_id: order.agent_id, maker_agent_id: maker_order.agent_id, maker_order_id: maker_order.id, taker_side: order.side });
                        maker_order.filled += trade_volume;
                        order.filled += trade_volume;
                        level.total_volume -= trade_volume;
                    }
                    if maker_order.filled >= maker_order.volume {
                        filled_order_ids.push(level.orders.pop_front().unwrap().id);
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
        for id in filled_order_ids {
            self.order_id_map.remove(&id);
        }
        if order.filled < order.volume {
            self.add_limit_order(*order);
        }
        trades
    }

    pub fn cancel_order(&mut self, order_id: u64, agent_id: usize) -> bool {
        if let Some(&(side, price)) = self.order_id_map.get(&order_id) {
            let book_side = match side {
                Side::Buy => &mut self.bids,
                Side::Sell => &mut self.asks,
            };
            if let Some(level) = book_side.get_mut(&price) {
                if let Some(index) = level.orders.iter().position(|o| o.id == order_id) {
                    if level.orders[index].agent_id == agent_id {
                        let cancelled_order = level.orders.remove(index).unwrap();
                        let remaining_volume = cancelled_order.volume.saturating_sub(cancelled_order.filled);
                        level.total_volume = level.total_volume.saturating_sub(remaining_volume);
                        if level.orders.is_empty() {
                            book_side.remove(&price);
                        }
                        self.order_id_map.remove(&order_id);
                        return true;
                    }
                }
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
        assert!(book.order_id_map.contains_key(&1), "Order ID should be in the index map.");
        let level = book.bids.get(&100).unwrap();
        assert_eq!(level.total_volume, 50);
    }

    #[test]
    fn test_market_order_simple_fill() {
        let mut book = OrderBook::new();
        book.add_limit_order(new_order(1, 1, Side::Sell, 100, 50));
        let trades = book.process_market_order(2, Side::Buy, 30);
        assert_eq!(trades.len(), 1);
        let ask_level = book.asks.get(&100).unwrap();
        assert_eq!(ask_level.total_volume, 20);
        assert_eq!(ask_level.orders[0].filled, 30);
    }

    #[test]
    fn test_market_order_full_fill_and_remove() {
        let mut book = OrderBook::new();
        book.add_limit_order(new_order(1, 1, Side::Sell, 100, 50));
        book.process_market_order(2, Side::Buy, 50);
        assert!(book.asks.get(&100).is_none(), "Price level should be removed after full fill.");
        assert!(book.order_id_map.get(&1).is_none(), "Order ID should be removed from the index map.");
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
        assert!(book.asks.get(&100).is_none());
        let bid_level = book.bids.get(&101).unwrap();
        assert_eq!(bid_level.total_volume, 20);
    }

    #[test]
    fn test_cancel_order_simple() {
        let mut book = OrderBook::new();
        book.add_limit_order(new_order(1, 1, Side::Buy, 100, 50));
        let success = book.cancel_order(1, 1);
        assert!(success);
        assert!(book.bids.is_empty());
        assert!(book.order_id_map.get(&1).is_none());
    }

    #[test]
    fn test_cancel_order_fails_for_wrong_owner() {
        let mut book = OrderBook::new();
        book.add_limit_order(new_order(1, 1, Side::Buy, 100, 50));
        let success = book.cancel_order(1, 2); // Agent 2 tries to cancel agent 1's order
        assert!(!success);
        assert_eq!(book.bids.get(&100).unwrap().total_volume, 50);
    }
    
    #[test]
    fn test_market_order_blows_through_multiple_levels() {
        let mut book = OrderBook::new();
        book.add_limit_order(new_order(1, 1, Side::Sell, 100, 20));
        book.add_limit_order(new_order(2, 1, Side::Sell, 101, 30));
        book.add_limit_order(new_order(3, 1, Side::Sell, 102, 40));
        
        let trades = book.process_market_order(2, Side::Buy, 100);
        
        assert_eq!(trades.len(), 3);
        assert_eq!(trades.iter().map(|t| t.volume).sum::<u64>(), 90);
        assert_eq!(trades[0].price, 100);
        assert_eq!(trades[1].price, 101);
        assert_eq!(trades[2].price, 102);
        assert!(book.asks.is_empty());
        assert!(book.order_id_map.is_empty());
    }
    
    #[test]
    fn test_partial_cancel_after_partial_fill() {
        let mut book = OrderBook::new();
        book.add_limit_order(new_order(1, 1, Side::Sell, 100, 100));
        
        book.process_market_order(2, Side::Buy, 40);
        
        let level = book.asks.get(&100).expect("Price level should still exist.");
        assert_eq!(level.total_volume, 60);
        assert_eq!(level.orders[0].filled, 40);

        let success = book.cancel_order(1, 1);
        
        assert!(success);
        assert!(book.asks.get(&100).is_none());
    }
}
