// src/stocks/definitions.rs
use serde::{Deserialize, Serialize};
//#[derive(Debug, Clone, Copy)]
pub type Symbol = String;

// Serializer is for saving and loading stock data.
// The `derive` macro automatically implements the Serialize and Deserialize traits.
#[derive(Debug, Clone, Serialize, Deserialize)] 
pub struct Stock {
    // Field renamed to 'symbol' for consistency with its usage in registry.rs
    pub symbol: Symbol,
    pub company_name: String,
    pub total_float: u64,
    pub initial_price: f64,
}
