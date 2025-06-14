// src/stock/definitions.rs
pub type Symbol = String;
// Serializer is for saving and loading stock data
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)] 
pub struct Stock {
    pub ticker: Symbol,
    pub company_name: String,
    pub total_float: u64,
    pub initial_price: f64,
}