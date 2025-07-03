// src/stocks/definitions.rs
//! Core stock metadata used by the simulator.

pub type Symbol = String;

use serde::{Deserialize, Serialize};

/// Immutable facts about a listed company.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stock {
    pub ticker: Symbol,
    pub id: u64,
    pub company_name: String,
    pub total_float: u64,
    pub initial_price: f64,
    pub sentiment_port: u64,
}

impl Stock {
    #[inline]
    pub fn new<T1: Into<String>, T2: Into<String>>(
        ticker: T1,
        id: u64,
        company_name: T2,
        total_float: u64,
        initial_price: f64,
        sentiment_port: u64,
    ) -> Self {
        Self {
            ticker: ticker.into(),
            id,
            company_name: company_name.into(),
            total_float,
            initial_price,
            sentiment_port,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StockMarket {
    pub stocks: Vec<Stock>,
    pub id_to_stock: std::collections::HashMap<u64, Stock>,
    pub ticker_to_stock: std::collections::HashMap<Symbol, Stock>,
}

#[inline]
pub fn default_stock_universe() -> Vec<Stock> {
    let file_path = "stock.csv".to_string();
    if std::path::Path::new(&file_path).exists() {
        let mut stocks = Vec::new();
        let mut rdr = csv::Reader::from_path(file_path).expect("Could not read stock.csv");
        for result in rdr.deserialize() {
            let stock: Stock = result.expect("Could not deserialize stock");
            stocks.push(stock);
        }
        stocks
    } else {
        vec![
            Stock::new("AAPL", 1, "Apple Inc.", 15_982_000_000, 195.37, 80),
            Stock::new(
                "MSFT",
                2,
                "Microsoft Corporation",
                7_448_000_000,
                422.12,
                80,
            ),
        ]
    }
}

#[inline]
pub fn stock_id_to_stock_map(stocks: &[Stock]) -> std::collections::HashMap<u64, Stock> {
    stocks.iter().map(|s| (s.id, s.clone())).collect()
}

#[inline]
pub fn stock_ticker_to_stock_map(stocks: &[Stock]) -> std::collections::HashMap<Symbol, Stock> {
    stocks
        .iter()
        .map(|s| (s.ticker.clone(), s.clone()))
        .collect()
}

impl StockMarket {
    pub fn new() -> Self {
        Self::from_universe(default_stock_universe())
    }

    pub fn from_universe(stocks: Vec<Stock>) -> Self {
        Self {
            id_to_stock: stock_id_to_stock_map(&stocks),
            ticker_to_stock: stock_ticker_to_stock_map(&stocks),
            stocks,
        }
    }

    pub fn get_stock_by_id(&self, id: u64) -> Option<&Stock> {
        self.id_to_stock.get(&id)
    }

    pub fn add_stock(&mut self, stock: Stock) {
        self.stocks.push(stock);
        self.id_to_stock = stock_id_to_stock_map(&self.stocks);
        self.ticker_to_stock = stock_ticker_to_stock_map(&self.stocks);
    }

    pub fn remove_stock(&mut self, id: u64) -> Option<()> {
        if let Some(pos) = self.stocks.iter().position(|s| s.id == id) {
            let removed_stock = self.stocks.remove(pos);
            self.id_to_stock.remove(&id);
            self.ticker_to_stock.remove(&removed_stock.ticker);
            Some(())
        } else {
            None
        }
    }

    pub fn update_stock(&mut self, id: u64, new_stock: Stock) -> Option<()> {
        if let Some(pos) = self.stocks.iter().position(|s| s.id == id) {
            self.stocks[pos] = new_stock.clone();
            self.id_to_stock.insert(id, new_stock.clone());
            self.ticker_to_stock
                .insert(new_stock.ticker.clone(), new_stock);
            Some(())
        } else {
            None
        }
    }

    pub fn get_stock_by_ticker(&self, ticker: &Symbol) -> Option<&Stock> {
        self.ticker_to_stock.get(ticker)
    }

    pub fn get_all_stocks(&self) -> Vec<&Stock> {
        self.stocks.iter().collect()
    }

    pub fn get_all_tickers(&self) -> Vec<Symbol> {
        self.stocks.iter().map(|s| s.ticker.clone()).collect()
    }

    pub fn get_all_ids(&self) -> Vec<u64> {
        self.stocks.iter().map(|s| s.id).collect()
    }

    pub fn get_ticker_by_id(&self, id: u64) -> Option<&Symbol> {
        self.id_to_stock.get(&id).map(|s| &s.ticker)
    }

    pub fn get_id_by_ticker(&self, ticker: &Symbol) -> Option<u64> {
        self.ticker_to_stock.get(ticker).map(|s| s.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_stock_universe() -> Vec<Stock> {
        vec![
            Stock::new("TEST1", 1, "Test Company 1", 1000, 10.0, 8001),
            Stock::new("TEST2", 2, "Test Company 2", 2000, 20.0, 8002),
        ]
    }

    fn create_test_stock_market() -> StockMarket {
        StockMarket::from_universe(test_stock_universe())
    }

    #[test]
    fn default_universe_is_consistent() {
        let sm = create_test_stock_market();
        assert_eq!(sm.stocks.len(), 2);
        assert_eq!(sm.id_to_stock.len(), 2);
        assert_eq!(sm.ticker_to_stock.len(), 2);

        for s in &sm.stocks {
            assert!(sm.get_stock_by_id(s.id).is_some());
            assert!(sm.get_stock_by_ticker(&s.ticker).is_some());
            assert_eq!(sm.get_ticker_by_id(s.id).unwrap(), &s.ticker);
            assert_eq!(sm.get_id_by_ticker(&s.ticker).unwrap(), s.id);
        }
    }

    #[test]
    fn add_stock_updates_all_structures() {
        let mut sm = create_test_stock_market();
        let extra = Stock::new("TEST3", 3, "Test Company 3", 3000, 30.0, 8003);
        sm.add_stock(extra.clone());

        assert_eq!(sm.stocks.len(), 3);
        let fetched = sm.get_stock_by_id(extra.id).unwrap();
        assert_eq!(fetched.ticker, extra.ticker);
        assert_eq!(sm.get_stock_by_ticker(&extra.ticker).unwrap().id, extra.id);
    }

    #[test]
    fn remove_stock_cleans_everywhere() {
        let mut sm = create_test_stock_market();
        let id_to_remove = 1;
        let initial_len = sm.stocks.len();

        assert!(sm.remove_stock(id_to_remove).is_some());
        assert_eq!(sm.stocks.len(), initial_len - 1);
        assert!(sm.get_stock_by_id(id_to_remove).is_none());
        assert!(sm.get_stock_by_ticker(&"TEST1".to_string()).is_none());
    }

    #[test]
    fn update_stock_reflects_in_maps() {
        let mut sm = create_test_stock_market();
        let mut edited = sm
            .get_stock_by_ticker(&"TEST1".to_string())
            .unwrap()
            .clone();
        edited.total_float = 9_999_999;

        assert!(sm.update_stock(edited.id, edited.clone()).is_some());
        let s_by_id = sm.get_stock_by_id(edited.id).unwrap();
        assert_eq!(s_by_id.total_float, 9_999_999);
        let s_by_tkr = sm.get_stock_by_ticker(&edited.ticker).unwrap();
        assert_eq!(s_by_tkr.total_float, 9_999_999);
    }

    #[test]
    fn get_functions_handle_nonexistent() {
        let sm = create_test_stock_market();
        assert!(sm.get_stock_by_id(999).is_none());
        assert!(sm.get_stock_by_ticker(&"ZZZZ".to_string()).is_none());
        assert!(sm.get_ticker_by_id(999).is_none());
        assert!(sm.get_id_by_ticker(&"ZZZZ".to_string()).is_none());
    }
}
