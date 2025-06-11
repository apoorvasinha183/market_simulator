// src/simulators/order_book.rs

use crate::types::order::Side;
use std::collections::BTreeMap;
use rand_distr::{Distribution as RandDistribution, Normal as RandNormal};
use statrs::distribution::{Continuous, Normal as StatNormal};

#[derive(Debug)]
pub struct Trade {
    pub price: u64,
    pub volume: u64,
}

pub struct OrderBook {
    pub bids: BTreeMap<u64, u64>,
    pub asks: BTreeMap<u64, u64>,
}

impl OrderBook {
    pub fn new_random() -> Self {
        let mut rng = rand::thread_rng();
        let center_price = 15000;
        let tick_size = 5;
        let mut bids = BTreeMap::new();
        let mut asks = BTreeMap::new();
        let ask_dist_mean = (center_price + 2 * tick_size) as f64;
        let ask_volume_sampler = RandNormal::new(1000.0, 400.0).unwrap();
        let ask_pdf_calculator = StatNormal::new(ask_dist_mean, 15.0 * tick_size as f64).unwrap();
        for i in 0..50 { let price = ask_dist_mean as u64 + i * tick_size; let probability_factor = ask_pdf_calculator.pdf(price as f64); let volume = (ask_volume_sampler.sample(&mut rng) * probability_factor * 100.0).max(0.0) as u64; if volume > 0 { asks.insert(price, volume); } }
        let bid_dist_mean = (center_price - 2 * tick_size) as f64;
        let bid_volume_sampler = RandNormal::new(1000.0, 400.0).unwrap();
        let bid_pdf_calculator = StatNormal::new(bid_dist_mean, 15.0 * tick_size as f64).unwrap();
        for i in 0..50 { let price = bid_dist_mean as u64 - i * tick_size; let probability_factor = bid_pdf_calculator.pdf(price as f64); let volume = (bid_volume_sampler.sample(&mut rng) * probability_factor * 100.0).max(0.0) as u64; if volume > 0 { bids.insert(price, volume); } }
        Self { bids, asks }
    }

    pub fn add_limit_order(&mut self, price: u64, volume: u64, side: Side) {
        let book_side = match side {
            Side::Buy => &mut self.bids,
            Side::Sell => &mut self.asks,
        };
        *book_side.entry(price).or_insert(0) += volume;
    }
    
    pub fn process_market_order(&mut self, side: Side, volume: u64) -> Vec<Trade> {
        let mut trades = Vec::new(); let mut volume_to_fill = volume;
        match side {
            Side::Buy => { let mut filled_price_levels = Vec::new(); for (&price, level_volume) in self.asks.iter_mut() { if volume_to_fill == 0 { break; } let trade_volume = volume_to_fill.min(*level_volume); trades.push(Trade { price, volume: trade_volume }); *level_volume -= trade_volume; volume_to_fill -= trade_volume; if *level_volume == 0 { filled_price_levels.push(price); } } for price in filled_price_levels { self.asks.remove(&price); } },
            Side::Sell => { let mut filled_price_levels = Vec::new(); for (&price, level_volume) in self.bids.iter_mut().rev() { if volume_to_fill == 0 { break; } let trade_volume = volume_to_fill.min(*level_volume); trades.push(Trade { price, volume: trade_volume }); *level_volume -= trade_volume; volume_to_fill -= trade_volume; if *level_volume == 0 { filled_price_levels.push(price); } } for price in filled_price_levels { self.bids.remove(&price); } }
        }
        trades
    }
}
