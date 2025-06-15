// src/simulators/market_trait.rs

use crate::simulators::order_book::OrderBook;
// NEW: Need Symbol for the method signature
use crate::stocks::definitions::Symbol;
use std::any::Any;
use std::collections::HashMap;

pub trait Marketable {
    fn step(&mut self) -> f64;
    // CHANGED: This method is now ambiguous and should take a symbol.
    fn current_price(&self, symbol: &Symbol) -> Option<f64>;
    fn reset(&mut self);
    // CHANGED: This method now must take a symbol.
    fn get_order_book(&self, symbol: &Symbol) -> Option<&OrderBook>;
    // NEW: A way to get all available order books.
    fn get_order_books(&self) -> &HashMap<Symbol, OrderBook>;
    fn as_any(&self) -> &dyn Any;
}
