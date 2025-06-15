// src/simulators/gbm.rs

use super::market_trait::Marketable;
// NEW: We need these types for the trait implementation
use crate::{stocks::definitions::Symbol, OrderBook};
use rand::distributions::Distribution;
use rand::rngs::ThreadRng;
use rand_distr::Normal;
use std::any::Any;
use std::collections::HashMap;

pub struct GBMSimulator {
    initial_price: f64,
    drift: f64,
    volatility: f64,
    current_price: f64,
    rng: ThreadRng,
    normal_dist: Normal<f64>,
    // NEW: Add an empty HashMap to satisfy the trait.
    dummy_books: HashMap<Symbol, OrderBook>,
}

impl GBMSimulator {
    pub fn new(initial_price: f64, drift: f64, volatility: f64) -> Self {
        Self {
            initial_price,
            drift,
            volatility,
            current_price: initial_price,
            rng: rand::thread_rng(),
            normal_dist: Normal::new(0.0, 1.0).unwrap(),
            dummy_books: HashMap::new(), // NEW
        }
    }
}

impl Marketable for GBMSimulator {
    fn step(&mut self) -> f64 {
        // ... (your existing GBM logic is perfect)
        let daily_drift = self.drift / 252.0;
        let daily_volatility = self.volatility / (252.0f64).sqrt();
        let dt = 1.0;
        let random_shock = self.normal_dist.sample(&mut self.rng);
        let next_price = self.current_price
            * ((daily_drift - 0.5 * daily_volatility.powi(2)) * dt
                + daily_volatility * random_shock * dt.sqrt())
            .exp();
        self.current_price = next_price;
        self.current_price
    }

    // FIXED: Signature now matches the trait. It ignores the symbol.
    fn current_price(&self, _symbol: &Symbol) -> Option<f64> {
        Some(self.current_price)
    }

    fn reset(&mut self) {
        self.current_price = self.initial_price;
        self.rng = rand::thread_rng();
    }
    
    // FIXED: Signature now matches the trait.
    fn get_order_book(&self, _symbol: &Symbol) -> Option<&OrderBook> {
        None // A GBM simulator doesn't have an order book.
    }

    // NEW: Implement the missing trait method.
    fn get_order_books(&self) -> &HashMap<Symbol, OrderBook> {
        &self.dummy_books
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
