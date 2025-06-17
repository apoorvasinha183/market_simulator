// src/stocks/definitions.rs
//! Core stock metadata used by the simulator.
//
//! For now we hard-code two symbols.  Extend `default_stock_universe()`
//! with more entries whenever you add tickers.

pub type Symbol = String;

use serde::{Deserialize, Serialize};

/// Immutable facts about a listed company. Adding a uniuqe stock ticker index to avoid the bs with String Copy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stock {
    /// NASDAQ / NYSE ticker (e.g. "AAPL").
    pub ticker: Symbol,
    /// Unique Stock Id:
    pub id: u64,
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
        id: u64,
        company_name: T2,
        total_float: u64,
        initial_price: f64,
    ) -> Self {
        Self {
            ticker: ticker.into(),
            id,
            company_name: company_name.into(),
            total_float,
            initial_price,
        }
    }
}
/// Maybe later we can have a Stock Mareket struct that holds a collection of stocks and their metadata.
/// We can then have a facility to add stocks to the market, remove them, and query for them.
#[derive(Debug)]
pub struct StockMarket {
    /// Collection of stocks available in the market.
    pub stocks: Vec<Stock>,
    /// A mapping from stock ID to stock for fast lookups
    pub id_to_stock: std::collections::HashMap<u64, Stock>,
    /// A mapping from stock ticekr to Stock for fast lookups
    pub ticker_to_stock: std::collections::HashMap<Symbol, Stock>,
}
/// The universe of stocks available when the market boots.
///
/// *Add or remove entries here to grow/shrink the simulation space. Right now I am fucking confused with IDs.*
#[inline]
pub fn default_stock_universe() -> Vec<Stock> {
    vec![
        Stock::new(
            "AAPL",
            1,
            "Apple Inc.",
            15_982_000_000, // float, not split-adjusted
            195.37,         // centre price at t=0
        ),
        Stock::new("MSFT", 2, "Microsoft Corporation", 7_448_000_000, 422.12),
    ]
}
/// Prepares a stock id to stock mapping for fast lookups.
/// This is used to quickly find a stock by its ID without iterating over the vector.
#[inline]
pub fn stock_id_to_stock_map(stocks: &[Stock]) -> std::collections::HashMap<u64, Stock> {
    stocks.iter().map(|s| (s.id, s.clone())).collect()
}
/// Prepare a stock ticker to Stock mapping for fast lookups.
#[inline]
pub fn stock_ticker_to_stock_map(stocks: &[Stock]) -> std::collections::HashMap<Symbol, Stock> {
    stocks
        .iter()
        .map(|s| (s.ticker.clone(), s.clone()))
        .collect()
}
impl StockMarket {
    /// Creates a new stock market with the default universe.
    pub fn new() -> Self {
        Self {
            stocks: default_stock_universe(),
            id_to_stock: stock_id_to_stock_map(&default_stock_universe()),
            ticker_to_stock: stock_ticker_to_stock_map(&default_stock_universe()),
        }
    }

    /// Returns a reference to the stock with the given ID, if it exists.
    pub fn get_stock_by_id(&self, id: u64) -> Option<&Stock> {
        self.id_to_stock.get(&id)
    }
    /// Add a new stock to the market.
    pub fn add_stock(&mut self, stock: Stock) {
        self.stocks.push(stock);
        // This is so infrequent that we can afford to rebuild the maps.
        self.id_to_stock = stock_id_to_stock_map(&self.stocks);
        self.ticker_to_stock = stock_ticker_to_stock_map(&self.stocks);
    }
    /// Remove a stock by its ID.
    pub fn remove_stock(&mut self, id: u64) -> Option<()> {
        // Three deletes : from the vector, from the ID map, and from the ticker map.
        if let Some(pos) = self.stocks.iter().position(|s| s.id == id) {
            // Remove the stock from the vector.
            let removed_stock = self.stocks.remove(pos);
            // Remove the stock from the ID map.
            self.id_to_stock.remove(&id);
            // Remove the stock from the ticker map.
            self.ticker_to_stock.remove(&removed_stock.ticker);
            Some(())
        } else {
            None
        }
    }
    /// Update stock information by ID.
    pub fn update_stock(&mut self, id: u64, new_stock: Stock) -> Option<()> {
        // tHREE updates in vector and the two maps
        if let Some(pos) = self.stocks.iter().position(|s| s.id == id) {
            // Update the stock in the vector.
            self.stocks[pos] = new_stock.clone();
            // Update the ID map.
            self.id_to_stock.insert(id, new_stock.clone());
            // Update the ticker map.
            self.ticker_to_stock
                .insert(new_stock.ticker.clone(), new_stock);
            Some(())
        } else {
            None
        }
    }
    /// Returns a reference to the stock with the given ticker, if it exists.
    pub fn get_stock_by_ticker(&self, ticker: &Symbol) -> Option<&Stock> {
        self.ticker_to_stock.get(ticker)
    }
    /// Returns a vector of all stocks in the market.
    pub fn get_all_stocks(&self) -> Vec<&Stock> {
        self.stocks.iter().collect()
    }
    /// Returns a vector of all stock tickers in the market.
    pub fn get_all_tickers(&self) -> Vec<Symbol> {
        self.stocks.iter().map(|s| s.ticker.clone()).collect()
    }
    /// Returns a vector of all stock IDs in the market.
    pub fn get_all_ids(&self) -> Vec<u64> {
        self.stocks.iter().map(|s| s.id).collect()
    }
    /// Return a ticker for a given stock ID, if it exists.
    pub fn get_ticker_by_id(&self, id: u64) -> Option<&Symbol> {
        self.id_to_stock.get(&id).map(|s| &s.ticker)
    }
    /// Return the ID for a given ticker, if it exists.
    pub fn get_id_by_ticker(&self, ticker: &Symbol) -> Option<u64> {
        self.ticker_to_stock.get(ticker).map(|s| s.id)
    }
}
// -----------------------------------------------------------------------------
//  Unit tests: StockMarket invariants
// -----------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    fn make_demo_stock() -> Stock {
        Stock::new("GOOG", 42, "Alphabet Inc.", 12_345_678_000, 1337.00)
    }

    #[test]
    fn default_universe_is_consistent() {
        let sm = StockMarket::new();
        // Size agreement
        assert_eq!(sm.stocks.len(), sm.id_to_stock.len());
        assert_eq!(sm.stocks.len(), sm.ticker_to_stock.len());

        // Every stock is reachable through both maps
        for s in &sm.stocks {
            assert!(sm.get_stock_by_id(s.id).is_some());
            assert!(sm.get_stock_by_ticker(&s.ticker).is_some());
            assert_eq!(sm.get_ticker_by_id(s.id).unwrap(), &s.ticker);
            assert_eq!(sm.get_id_by_ticker(&s.ticker).unwrap(), s.id);
        }
    }

    #[test]
    fn add_stock_updates_all_structures() {
        let mut sm = StockMarket::new();
        let extra = make_demo_stock();
        sm.add_stock(extra.clone());

        assert_eq!(sm.stocks.len(), 3);
        let fetched = sm.get_stock_by_id(extra.id).unwrap();
        assert_eq!(fetched.ticker, extra.ticker);
        assert_eq!(sm.get_stock_by_ticker(&extra.ticker).unwrap().id, extra.id);
    }

    #[test]
    fn remove_stock_cleans_everywhere() {
        let mut sm = StockMarket::new();
        let extra = make_demo_stock();
        sm.add_stock(extra.clone());

        assert!(sm.remove_stock(extra.id).is_some());
        assert!(sm.get_stock_by_id(extra.id).is_none());
        assert!(sm.get_stock_by_ticker(&extra.ticker).is_none());
        assert_eq!(sm.stocks.len(), 2);
    }

    #[test]
    fn update_stock_reflects_in_maps() {
        let mut sm = StockMarket::new();
        let mut edited = sm.get_stock_by_ticker(&"AAPL".to_string()).unwrap().clone();
        edited.total_float = 9_999_999;

        assert!(sm.update_stock(edited.id, edited.clone()).is_some());
        let s_by_id = sm.get_stock_by_id(edited.id).unwrap();
        assert_eq!(s_by_id.total_float, 9_999_999);
        let s_by_tkr = sm.get_stock_by_ticker(&edited.ticker).unwrap();
        assert_eq!(s_by_tkr.total_float, 9_999_999);
    }

    #[test]
    fn get_functions_handle_nonexistent() {
        let sm = StockMarket::new();
        assert!(sm.get_stock_by_id(999).is_none());
        assert!(sm.get_stock_by_ticker(&"ZZZZ".to_string()).is_none());
        assert!(sm.get_ticker_by_id(999).is_none());
        assert!(sm.get_id_by_ticker(&"ZZZZ".to_string()).is_none());
    }
}
