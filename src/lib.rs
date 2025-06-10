// src/lib.rs

// === 1. Declare the top-level modules ===
// This makes `pricing::`, `simulators::`, and `shared_types::` available within the library.
pub mod pricing;
pub mod simulators;
pub mod shared_types;


// === 2. Re-export the public-facing components to create a clean API ===
// We want users of our library to be able to write `market_simulator::Component`
// to get access to these important structs and traits.

// --- From the `simulators` module ---
// The core trait that all simulators must implement.
pub use simulators::market_trait::Marketable;
// The first concrete simulator we built.
pub use simulators::gbm::GBMSimulator;
// The new order book simulator and its supporting types.
//pub use simulators::order_book::{OrderBook, OrderBookSimulator, Side, Trade};


// --- From the `pricing` module ---
// The new, stateful option pricer is the main public component.
pub use pricing::OptionPricer;
// The data structure for returning the Greeks.
pub use pricing::Greeks;


// --- From the `shared_types` module ---
// The enum to define call or put options.
pub use shared_types::OptionType;