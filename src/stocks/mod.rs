// src/stocks/mod.rs
// -----------------
pub mod definitions;

// Re-export the most useful items so callers don’t have to dive
// another level down the path.
pub use definitions::{Stock, StockMarket, Symbol, default_stock_universe};
