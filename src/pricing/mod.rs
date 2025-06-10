// src/pricing/mod.rs

// This line finds and includes the `black_scholes.rs` file as a module.
pub mod black_scholes;

// These lines make the important structs from the inner module
// directly available from the parent `pricing` module.
pub use black_scholes::Greeks;
pub use black_scholes::OptionPricer;