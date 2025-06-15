// src/stocks/definitions.rs
//! Core stock metadata used by the simulator.
//
//! For now we hard-code two symbols.  Extend `default_stock_universe()`
//! with more entries whenever you add tickers.

pub type Symbol = String;

use serde::{Deserialize, Serialize};

/// Immutable facts about a listed company.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stock {
    /// NASDAQ / NYSE ticker (e.g. "AAPL").
    pub ticker: Symbol,
    /// Human-readable company name.
    pub company_name: String,
    /// Shares available for trading.
    pub total_float: u64,
    /// Opening mid-price at time-zero of the simulation.
    pub initial_price: f64,
}

/// Convenience factory so call-sites stay concise.
impl Stock {
    #[inline]
    pub fn new<T1: Into<String>, T2: Into<String>>(
        ticker: T1,
        company_name: T2,
        total_float: u64,
        initial_price: f64,
    ) -> Self {
        Self {
            ticker: ticker.into(),
            company_name: company_name.into(),
            total_float,
            initial_price,
        }
    }
}

/// The universe of stocks available when the market boots.
///
/// *Add or remove entries here to grow/shrink the simulation space.*
#[inline]
pub fn default_stock_universe() -> Vec<Stock> {
    vec![
        Stock::new(
            "AAPL",
            "Apple Inc.",
            15_982_000_000, // float, not split-adjusted
            195.37,         // centre price at t=0
        ),
        Stock::new(
            "MSFT",
            "Microsoft Corporation",
            7_448_000_000,
            422.12,
        ),
    ]
}
