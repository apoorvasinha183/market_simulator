// src/lib.rs

// Declare the modules
pub mod pricing;
pub mod simulators;
pub mod shared_types;

// Re-export the key components so they are easily accessible from the outside.
pub use simulators::gbm::GBMSimulator;
pub use simulators::market_trait::Marketable;
pub use pricing::black_scholes::calculate_option_price;
pub use shared_types::OptionType;