// src/stocks/definitions.rs
//! Core stock metadata used by the simulator.

pub type Symbol = String;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Immutable facts about a listed company or ETF.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stock {
    pub ticker: Symbol,
    pub id: u64,
    pub company_name: String,
    pub total_float: u64,
    pub initial_price: f64,
    pub sentiment_port: u64,
    #[serde(default)]
    pub ownership_allocation: String, // Raw CSV string like "mm:0.20,whale:0.30,thermo:0.35,momentum:0.15"
    #[serde(skip)]
    pub parsed_allocation: HashMap<String, f64>, // Parsed allocation map
}

/// ETF-specific information loaded from configs/etfs.csv
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ETFInfo {
    pub symbol: String,
    pub name: String,
    pub expense_ratio: f64,
    pub creation_unit_size: u64,
    pub holdings: String, // Raw holdings string like "GIGA:0.12,STONKS:0.10,DIAMOND:0.08"
    #[serde(skip)]
    pub parsed_holdings: HashMap<String, f64>, // Parsed holdings map: ticker -> weight
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
            ownership_allocation: String::new(),
            parsed_allocation: HashMap::new(),
        }
    }

    /// Parse the ownership allocation string and populate the parsed_allocation HashMap
    pub fn parse_ownership_allocation(&mut self) {
        self.parsed_allocation.clear();

        if self.ownership_allocation.is_empty() {
            return;
        }

        for allocation in self.ownership_allocation.split(',') {
            let parts: Vec<&str> = allocation.split(':').collect();
            if parts.len() == 2 {
                let agent_type = parts[0].trim().to_string();
                if let Ok(percentage) = parts[1].trim().parse::<f64>() {
                    self.parsed_allocation.insert(agent_type, percentage);
                }
            }
        }
    }

    /// Get the ownership allocation for a specific agent type
    pub fn get_allocation_for_agent(&self, agent_type: &str) -> f64 {
        self.parsed_allocation
            .get(agent_type)
            .copied()
            .unwrap_or(0.0)
    }

    /// Get all agent types that have ownership in this stock
    pub fn get_owner_agent_types(&self) -> Vec<String> {
        self.parsed_allocation.keys().cloned().collect()
    }

    /// Calculate shares owned by a specific agent type
    pub fn calculate_shares_for_agent(&self, agent_type: &str) -> u64 {
        let percentage = self.get_allocation_for_agent(agent_type);
        (self.total_float as f64 * percentage) as u64
    }

    /// Check if this stock is an ETF (based on ownership allocation)
    pub fn is_etf(&self) -> bool {
        self.parsed_allocation.contains_key("etf")
    }
}

impl ETFInfo {
    /// Parse the holdings string and populate the parsed_holdings HashMap
    pub fn parse_holdings(&mut self) {
        self.parsed_holdings.clear();

        if self.holdings.is_empty() {
            return;
        }

        for holding in self.holdings.split(',') {
            let parts: Vec<&str> = holding.split(':').collect();
            if parts.len() == 2 {
                let ticker = parts[0].trim().to_string();
                if let Ok(weight) = parts[1].trim().parse::<f64>() {
                    self.parsed_holdings.insert(ticker, weight);
                }
            }
        }
    }

    /// Get the weight of a specific holding in this ETF
    pub fn get_holding_weight(&self, ticker: &str) -> f64 {
        self.parsed_holdings.get(ticker).copied().unwrap_or(0.0)
    }

    /// Get all tickers held by this ETF
    pub fn get_holding_tickers(&self) -> Vec<String> {
        self.parsed_holdings.keys().cloned().collect()
    }

    /// Calculate NAV (Net Asset Value) based on current stock prices
    pub fn calculate_nav(&self, stock_market: &StockMarket) -> Option<f64> {
        let mut nav = 0.0;

        for (ticker, weight) in &self.parsed_holdings {
            if let Some(stock) = stock_market.get_stock_by_ticker(ticker) {
                nav += stock.initial_price * weight;
            } else {
                // If we can't find a holding, NAV calculation fails
                return None;
            }
        }

        Some(nav)
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct StockMarket {
    pub stocks: Vec<Stock>,
    pub id_to_stock: std::collections::HashMap<u64, Stock>,
    pub ticker_to_stock: std::collections::HashMap<Symbol, Stock>,
    pub etf_info: std::collections::HashMap<Symbol, ETFInfo>, // ETF-specific information
}

#[inline]
pub fn default_stock_universe() -> Vec<Stock> {
    let file_path = "stock.csv".to_string();
    if std::path::Path::new(&file_path).exists() {
        let mut stocks = Vec::new();
        let mut rdr = csv::Reader::from_path(file_path).expect("Could not read stock.csv");
        for result in rdr.deserialize() {
            let mut stock: Stock = result.expect("Could not deserialize stock");
            // Parse the ownership allocation after loading from CSV
            stock.parse_ownership_allocation();
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

impl Default for StockMarket {
    fn default() -> Self {
        Self::new()
    }
}

impl StockMarket {
    pub fn new() -> Self {
        Self::from_universe(default_stock_universe())
    }

    pub fn from_universe(stocks: Vec<Stock>) -> Self {
        let etf_info = Self::load_etf_info();
        Self {
            id_to_stock: stock_id_to_stock_map(&stocks),
            ticker_to_stock: stock_ticker_to_stock_map(&stocks),
            stocks,
            etf_info,
        }
    }

    /// Load ETF information from configs/etfs.csv
    fn load_etf_info() -> HashMap<Symbol, ETFInfo> {
        let mut etf_map = HashMap::new();
        let file_path = "configs/etfs.csv";

        if std::path::Path::new(file_path).exists() {
            if let Ok(mut rdr) = csv::Reader::from_path(file_path) {
                for result in rdr.deserialize() {
                    if let Ok(etf_info) = result {
                        let mut etf_info: ETFInfo = etf_info;
                        etf_info.parse_holdings();
                        etf_map.insert(etf_info.symbol.clone(), etf_info);
                    }
                }
            }
        }

        etf_map
    }

    /// Get ETF information by symbol
    pub fn get_etf_info(&self, symbol: &str) -> Option<&ETFInfo> {
        self.etf_info.get(symbol)
    }

    /// Get all ETF symbols
    pub fn get_all_etf_symbols(&self) -> Vec<String> {
        self.etf_info.keys().cloned().collect()
    }

    /// Get all ETFs (stocks that are ETFs)
    pub fn get_all_etfs(&self) -> Vec<&Stock> {
        self.stocks.iter().filter(|stock| stock.is_etf()).collect()
    }

    /// Calculate NAV for an ETF
    pub fn calculate_etf_nav(&self, etf_symbol: &str) -> Option<f64> {
        if let Some(etf_info) = self.get_etf_info(etf_symbol) {
            etf_info.calculate_nav(self)
        } else {
            None
        }
    }

    /// Check if a stock symbol is an ETF
    pub fn is_etf(&self, symbol: &str) -> bool {
        if let Some(stock) = self.get_stock_by_ticker(&symbol.to_string()) {
            stock.is_etf()
        } else {
            false
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

    /// Calculate initial inventory for a specific agent type across all stocks
    pub fn calculate_initial_inventory_for_agent(&self, agent_type: &str) -> HashMap<u64, u64> {
        let mut inventory = HashMap::new();

        for stock in &self.stocks {
            let shares = stock.calculate_shares_for_agent(agent_type);
            // Always include the stock in the inventory map, even if shares = 0
            inventory.insert(stock.id, shares);
        }

        inventory
    }

    /// Get all agent types that have ownership across the stock universe
    pub fn get_all_owner_agent_types(&self) -> std::collections::HashSet<String> {
        let mut agent_types = std::collections::HashSet::new();

        for stock in &self.stocks {
            for agent_type in stock.get_owner_agent_types() {
                agent_types.insert(agent_type);
            }
        }

        agent_types
    }

    /// Validate that ownership allocations sum to approximately 1.0 for each stock
    pub fn validate_ownership_allocations(&self) -> Result<(), String> {
        for stock in &self.stocks {
            let total: f64 = stock.parsed_allocation.values().sum();

            // Allow small floating point errors
            if (total - 1.0).abs() > 0.01 {
                return Err(format!(
                    "Stock {} ownership allocation sums to {:.3}, expected ~1.0",
                    stock.ticker, total
                ));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_stock_universe() -> Vec<Stock> {
        let mut stock1 = Stock::new("TEST1", 1, "Test Company 1", 1000, 10.0, 8001);
        stock1.ownership_allocation = "mm:0.5,whale:0.3,thermo:0.2".to_string();
        stock1.parse_ownership_allocation();

        let mut stock2 = Stock::new("TEST2", 2, "Test Company 2", 2000, 20.0, 8002);
        stock2.ownership_allocation = "mm:0.4,thermo:0.6".to_string();
        stock2.parse_ownership_allocation();

        vec![stock1, stock2]
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

    #[test]
    fn ownership_allocation_parsing_works() {
        let sm = create_test_stock_market();

        // Test stock 1: "mm:0.5,whale:0.3,thermo:0.2"
        let stock1 = sm.get_stock_by_id(1).unwrap();
        assert_eq!(stock1.get_allocation_for_agent("mm"), 0.5);
        assert_eq!(stock1.get_allocation_for_agent("whale"), 0.3);
        assert_eq!(stock1.get_allocation_for_agent("thermo"), 0.2);
        assert_eq!(stock1.get_allocation_for_agent("nonexistent"), 0.0);

        // Test stock 2: "mm:0.4,thermo:0.6"
        let stock2 = sm.get_stock_by_id(2).unwrap();
        assert_eq!(stock2.get_allocation_for_agent("mm"), 0.4);
        assert_eq!(stock2.get_allocation_for_agent("thermo"), 0.6);
        assert_eq!(stock2.get_allocation_for_agent("whale"), 0.0);
    }

    #[test]
    fn calculate_shares_for_agent_works() {
        let sm = create_test_stock_market();

        // Stock 1: 1000 total_float, mm gets 50%
        let stock1 = sm.get_stock_by_id(1).unwrap();
        assert_eq!(stock1.calculate_shares_for_agent("mm"), 500);
        assert_eq!(stock1.calculate_shares_for_agent("whale"), 300);
        assert_eq!(stock1.calculate_shares_for_agent("thermo"), 200);

        // Stock 2: 2000 total_float, mm gets 40%
        let stock2 = sm.get_stock_by_id(2).unwrap();
        assert_eq!(stock2.calculate_shares_for_agent("mm"), 800);
        assert_eq!(stock2.calculate_shares_for_agent("thermo"), 1200);
    }

    #[test]
    fn calculate_initial_inventory_for_agent_works() {
        let sm = create_test_stock_market();

        let mm_inventory = sm.calculate_initial_inventory_for_agent("mm");
        assert_eq!(mm_inventory.get(&1), Some(&500)); // 50% of 1000
        assert_eq!(mm_inventory.get(&2), Some(&800)); // 40% of 2000

        let thermo_inventory = sm.calculate_initial_inventory_for_agent("thermo");
        assert_eq!(thermo_inventory.get(&1), Some(&200)); // 20% of 1000
        assert_eq!(thermo_inventory.get(&2), Some(&1200)); // 60% of 2000

        let whale_inventory = sm.calculate_initial_inventory_for_agent("whale");
        assert_eq!(whale_inventory.get(&1), Some(&300)); // 30% of 1000
        assert_eq!(whale_inventory.get(&2), None); // No whale ownership in stock 2
    }
}
