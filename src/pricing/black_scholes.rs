// src/pricing/black_scholes.rs

use crate::shared_types::OptionType;
use statrs::distribution::{Continuous, ContinuousCDF, Normal as StatNormal};
use std::collections::VecDeque;

#[derive(Debug, Default, Clone, Copy)]
pub struct Greeks {
    /// The rate of change of the option price with respect to a $1 change in the stock price.
    pub delta: f64,
    /// The rate of change of Delta. Measures the convexity of the option price.
    pub gamma: f64,
    /// The rate of change of the option price with respect to a 1% change in volatility.
    pub vega: f64,
    /// The rate of change of the option price with respect to the passage of time (time decay).
    pub theta: f64,
    /// The rate of change of the option price with respect to a 1% change in the risk-free rate.
    pub rho: f64,
}

struct VolatilityEstimator {
    prices: VecDeque<f64>,
    window_size: usize,
    initial_volatility: f64,
}

impl VolatilityEstimator {
    fn new(window_size: usize, initial_volatility: f64) -> Self {
        Self {
            prices: VecDeque::with_capacity(window_size),
            window_size,
            initial_volatility,
        }
    }

    fn update(&mut self, new_price: f64) {
        if self.prices.len() == self.window_size {
            self.prices.pop_front();
        }
        self.prices.push_back(new_price);
    }

    fn calculate(&self) -> f64 {
        if self.prices.len() < self.window_size {
            return self.initial_volatility;
        }
        let returns: Vec<f64> = self.prices.as_slices().0.windows(2).map(|w| (w[1] / w[0]).ln()).collect();
        let num_returns = returns.len() as f64;
        if num_returns == 0.0 { return self.initial_volatility; }
        
        let mean_return = returns.iter().sum::<f64>() / num_returns;
        let variance = returns.iter().map(|r| (r - mean_return).powi(2)).sum::<f64>() / num_returns;
        
        variance.sqrt() * (252.0 as f64).sqrt()
    }
}

pub struct OptionPricer {
    volatility_estimator: VolatilityEstimator,
    option_type: OptionType,
    strike_price: f64,
    time_to_expiration: f64,
    risk_free_rate: f64,
}

impl OptionPricer {
    pub fn new(
        option_type: OptionType,
        strike_price: f64,
        time_to_expiration_years: f64,
        risk_free_rate: f64,
        initial_volatility: f64,
        volatility_window: usize,
    ) -> Self {
        Self {
            volatility_estimator: VolatilityEstimator::new(volatility_window, initial_volatility),
            option_type,
            strike_price,
            time_to_expiration: time_to_expiration_years,
            risk_free_rate,
        }
    }

    pub fn calculate_price_and_greeks(&mut self, stock_price: f64, days_elapsed: u32) -> (f64, Greeks) {
        self.volatility_estimator.update(stock_price);
        let dynamic_vol = self.volatility_estimator.calculate();
        let time_remaining = self.time_to_expiration - (days_elapsed as f64 / 252.0);

        if time_remaining <= 0.0 {
            let price = match self.option_type {
                OptionType::Call => (stock_price - self.strike_price).max(0.0),
                OptionType::Put => (self.strike_price - stock_price).max(0.0),
            };
            return (price, Greeks::default());
        }

        let normal = StatNormal::new(0.0, 1.0).unwrap();
        let d1 = ( (stock_price / self.strike_price).ln() + (self.risk_free_rate + 0.5 * dynamic_vol.powi(2)) * time_remaining )
                 / (dynamic_vol * time_remaining.sqrt());
        let d2 = d1 - dynamic_vol * time_remaining.sqrt();
        
        let theoretical_price = match self.option_type {
            OptionType::Call => stock_price * normal.cdf(d1) - self.strike_price * (-self.risk_free_rate * time_remaining).exp() * normal.cdf(d2),
            OptionType::Put => self.strike_price * (-self.risk_free_rate * time_remaining).exp() * normal.cdf(-d2) - stock_price * normal.cdf(-d1),
        };

        // --- NEW RULES APPLIED HERE ---

        // 1. Enforce a minimum price of one cent before expiration.
        let price_with_floor = theoretical_price.max(0.01);

        // 2. Quantize the data output by rounding to the nearest cent.
        let final_price = (price_with_floor * 100.0).round() / 100.0;
        
        // --- END OF NEW RULES ---

        

        let greeks = self.calculate_greeks(stock_price, d1, time_remaining, dynamic_vol);
        (final_price, greeks)
    }

    fn calculate_greeks(&self, stock_price: f64, d1: f64, time_remaining: f64, volatility: f64) -> Greeks {
        let normal = StatNormal::new(0.0, 1.0).unwrap();
        let n_d1_pdf = normal.pdf(d1);
        let d2 = d1 - volatility * time_remaining.sqrt();

        // --- DELTA ---
        let delta = match self.option_type {
            OptionType::Call => normal.cdf(d1),
            OptionType::Put => normal.cdf(d1) - 1.0,
        };

        // --- GAMMA ---
        let gamma = n_d1_pdf / (stock_price * volatility * time_remaining.sqrt());
        
        // --- VEGA --- (per 1% change in volatility)
        let vega = stock_price * n_d1_pdf * time_remaining.sqrt() * 0.01;

        // --- THETA --- (per day)
        let theta = match self.option_type {
            OptionType::Call => {
                -((stock_price * n_d1_pdf * volatility) / (2.0 * time_remaining.sqrt()))
                - self.risk_free_rate * self.strike_price * (-self.risk_free_rate * time_remaining).exp() * normal.cdf(d2)
            },
            OptionType::Put => {
                -((stock_price * n_d1_pdf * volatility) / (2.0 * time_remaining.sqrt()))
                + self.risk_free_rate * self.strike_price * (-self.risk_free_rate * time_remaining).exp() * normal.cdf(-d2)
            },
        } / 365.0;

        // --- RHO --- (per 1% change in risk-free rate)
        let rho = match self.option_type {
            OptionType::Call => self.strike_price * time_remaining * (-self.risk_free_rate * time_remaining).exp() * normal.cdf(d2) * 0.01,
            OptionType::Put => -self.strike_price * time_remaining * (-self.risk_free_rate * time_remaining).exp() * normal.cdf(-d2) * 0.01,
        };

        Greeks { delta, gamma, vega, theta, rho }
    }
}