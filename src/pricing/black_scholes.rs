// src/pricing/black_scholes.rs

use crate::shared_types::OptionType;
use statrs::distribution::{ContinuousCDF, Normal as StatNormal};

// The function signature is the same, but it now correctly uses the shared OptionType
pub fn calculate_option_price(
    option_type: OptionType,
    stock_price: f64,
    strike_price: f64,
    time_to_expiration: f64,
    risk_free_rate: f64,
    volatility: f64,
) -> f64 {
    if time_to_expiration <= 0.0 {
        return match option_type {
            OptionType::Call => (stock_price - strike_price).max(0.0),
            OptionType::Put => (strike_price - stock_price).max(0.0),
        };
    }

    let normal = StatNormal::new(0.0, 1.0).unwrap();
    let d1 = ( (stock_price / strike_price).ln() + (risk_free_rate + 0.5 * volatility.powi(2)) * time_to_expiration )
             / (volatility * time_to_expiration.sqrt());
    let d2 = d1 - volatility * time_to_expiration.sqrt();

    match option_type {
        OptionType::Call => {
            stock_price * normal.cdf(d1) - strike_price * (-risk_free_rate * time_to_expiration).exp() * normal.cdf(d2)
        }
        OptionType::Put => {
            strike_price * (-risk_free_rate * time_to_expiration).exp() * normal.cdf(-d2) - stock_price * normal.cdf(-d1)
        }
    }
}