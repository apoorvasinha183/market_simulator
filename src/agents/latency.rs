// src/agents/latency.rs

//! Agent startup latencies (in number of ticks).
//! This simulates the "warm-up" time for different classes of participants.

/// The Market Maker should be the fastest, waking up almost instantly.
pub const MM_TICKS_UNTIL_ACTIVE: u32 = 2;

/// Limit order traders are slower to react.
pub const LIMIT_AGENT_TICKS_UNTIL_ACTIVE: u32 = 10;

/// Market order traders are the slowest to join the market.
pub const DUMB_AGENT_TICKS_UNTIL_ACTIVE: u32 = 15;
///  Whaaaaaleeee
pub const WHALE_TICKS_UNTIL_ACTIVE: u32 = 20;
