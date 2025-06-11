// src/lib.rs

// === 1. Declare all the top-level modules ===
pub mod agents;
pub mod market;
pub mod pricing;
pub mod simulators;
pub mod types;
pub mod shared_types;


// === 2. Re-export the public-facing components to create a clean API ===

// --- From `agents` ---
pub use agents::agent_trait::{Agent, MarketView};
pub use agents::agent_type::AgentType; // <-- EXPORT THE NEW ENUM
pub use agents::dumb_agent::DumbAgent;
pub use agents::dumb_limit_agent::DumbLimitAgent;
pub use agents::market_maker_agent::MarketMakerAgent;
pub use agents::ipo_agent::IpoAgent;

// --- From our `market` engine ---
pub use market::Market;

// --- From `simulators` ---
pub use simulators::market_trait::Marketable;
pub use simulators::gbm::GBMSimulator;
pub use simulators::order_book::{OrderBook, Trade};

// --- From `pricing` ---
pub use pricing::{OptionPricer, Greeks};

// --- From `types` ---
pub use types::order::{Order,OrderRequest, Side};
//pub use types::order::{Order, OrderRequest, Side};
// --- From `shared_types` ---
pub use shared_types::OptionType;
