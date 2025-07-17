pub mod agent_trait;
pub mod agent_type;
pub mod astrologer_agent;
pub mod web_server;
pub mod config;
pub mod customer_agent;
pub mod dumb_agent;
pub mod dumb_limit_agent;
pub mod ipo_agent;
pub mod latency;
pub mod market_maker_agent;
pub mod momentum_agent;
pub mod thermo_agent;
pub mod whale_agent;
pub mod web_proxy_agent;

/// Quantizes a price to the nearest 5-cent increment.
#[inline]
pub fn quantize_price(price: u64) -> u64 {
    price
}
