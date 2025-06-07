// src/simulators/market_trait.rs

/// A trait for any object that can simulate a market price.
/// This allows for a pluggable simulation engine.
pub trait Marketable {
    /// Advances the simulation by one time step and returns the new price.
    fn step(&mut self) -> f64;
    
    /// Returns the current price without advancing the simulation.
    fn current_price(&self) -> f64;

    /// Resets the simulation to its initial state.
    fn reset(&mut self);
}