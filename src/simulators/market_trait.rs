// src/simulators/market_trait.rs

use crate::stocks::definitions::Symbol;
use crate::simulators::order_book::OrderBook;
use std::collections::HashMap;
use std::any::Any;

pub struct MarketView<'a> {
    pub order_books: &'a HashMap<Symbol, OrderBook>,
    pub last_traded_prices: &'a HashMap<Symbol, f64>,
}

// FIXED: This is the correct location for the get_mid_price method.
// Adding this implementation block resolves all 9 "no method named `get_mid_price`" errors.
impl MarketView<'_> {
    pub fn get_mid_price(&self, symbol: &Symbol) -> Option<u64> {
       if let Some(order_book) = self.order_books.get(symbol) {
           let best_bid = order_book.get_bids().keys().last();
           let best_ask = order_book.get_asks().keys().next();

           match (best_bid, best_ask) {
               (Some(bid), Some(ask)) => Some((bid + ask) / 2),
               (Some(bid), None) => Some(*bid),
               (None, Some(ask)) => Some(*ask),
               (None, None) => self.last_traded_prices.get(symbol).map(|&p| (p * 100.0) as u64)
           }
       } else {
            self.last_traded_prices.get(symbol).map(|&p| (p * 100.0) as u64)
       }
   }
}

pub trait Marketable {
    fn step(&mut self) -> f64;
    fn current_price(&self, symbol: &Symbol) -> Option<f64>;
    fn get_order_book(&self, symbol: &Symbol) -> Option<&OrderBook>;
    fn get_order_books(&self) -> &HashMap<Symbol, OrderBook>;
    fn reset(&mut self);
    fn as_any(&self) -> &dyn Any;
}