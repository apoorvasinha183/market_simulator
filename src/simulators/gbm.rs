// src/simulators/gbm.rs

use rand::distributions::Distribution;
use rand_distr::Normal; 
use rand::rngs::ThreadRng;
use super::market_trait::Marketable; // <-- Import the trait

pub struct GBMSimulator {
    initial_price: f64,
    drift: f64,
    volatility: f64,
    current_price: f64,
    rng: ThreadRng,
    normal_dist: Normal<f64>,
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
        }
    }
}

// Here we provide the implementation of the Marketable trait for our GBM struct.
impl Marketable for GBMSimulator {
    // This one will generate the new prices for the order book.
    fn step(&mut self) -> f64 {
        let daily_drift = self.drift / 252.0;
        let daily_volatility = self.volatility / (252.0 as f64).sqrt();
        let dt = 1.0;
        let random_shock = self.normal_dist.sample(&mut self.rng);
        // Code below didn't have mu-sigma^2/2 term
        let next_price = self.current_price
            * ((daily_drift - 0.5 * daily_volatility.powi(2)) * dt + daily_volatility * random_shock * dt.sqrt()).exp();
        self.current_price = next_price;
        self.current_price
    }
    // This API is for observers in the market. That's you 🫵 you degenerate.
    fn current_price(&self) -> f64 {
        self.current_price
    }

    fn reset(&mut self) {
        self.current_price = self.initial_price;
        self.rng = rand::thread_rng(); // Get a new seed on reset
    }
}