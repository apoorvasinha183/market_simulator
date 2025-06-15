// src/stocks/registry.rs
use super::definitions::{Stock};

// This function will define the entire tradable universe for a simulation run.
pub fn get_tradable_universe() -> Vec<Stock> {
    vec![
        Stock {
            symbol: "AAPL".to_string(), // Corrected from APPL
            company_name: "Apple Inc.".to_string(), // Corrected name
            total_float: 1_000_000_000,
            initial_price: 196.00,
        },
        Stock {
            symbol: "QQQ".to_string(),
            company_name: "Invesco QQQ Trust".to_string(), // Corrected name
            total_float: 500_000_000,
            initial_price: 525.0, // Corrected to be a float
        },
        // Add more stocks here...
    ]
}
