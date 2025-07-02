pub mod agent_trait;
pub mod agent_type;
pub mod config;
pub mod dumb_agent;
pub mod dumb_limit_agent;
pub mod ipo_agent;
pub mod latency;
pub mod market_maker_agent;
pub mod whale_agent;
pub mod customer_agent;

/// Quantizes a price to the nearest 5-cent increment.
#[inline]
pub fn quantize_price(price: u64) -> u64 {
    (price / 5) * 5
}