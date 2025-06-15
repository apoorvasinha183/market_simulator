// src/lib.rs

pub mod agents;
pub mod market;
pub mod pricing;
pub mod shared_types;
pub mod simulators;
pub mod stocks;
pub mod types;

// Re-export key components for easier access in binaries and other crates.
pub use agents::{
    agent_trait::Agent, agent_type::AgentType, dumb_agent::DumbAgent,
    dumb_limit_agent::DumbLimitAgent, ipo_agent::IpoAgent, market_maker_agent::MarketMakerAgent,
    whale_agent::WhaleAgent,
};

pub use market::Market;

// FIXED: Correctly re-exporting MarketView and Marketable.
pub use simulators::{
    market_trait::{MarketView, Marketable},
    order_book::OrderBook,
};

pub use stocks::{
    definitions::{Stock, Symbol},
    registry,
};

// FIXED: Correctly re-exporting all types from the order module.
pub use types::order::{Order, OrderRequest, Side, Trade};