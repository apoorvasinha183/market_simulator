// src/agents/config.rs

// --- DumbAgent ---
pub const DUMB_AGENT_ACTION_PROB: f64 = 0.8;
pub const DUMB_AGENT_NUM_TRADERS: u32 = 10;
pub const DUMB_AGENT_TYPICAL_VOL_MIN: u64 = 100;
pub const DUMB_AGENT_TYPICAL_VOL_MAX: u64 = 200;
pub const DUMB_AGENT_LARGE_VOL_MIN: u64 = 500;
pub const DUMB_AGENT_LARGE_VOL_MAX: u64 = 1000;
pub const DUMB_AGENT_LARGE_VOL_CHANCE: f64 = 0.1;

// --- DumbLimitAgent ---
pub const LIMIT_AGENT_ACTION_PROB: f64 = 0.7;
pub const LIMIT_AGENT_NUM_TRADERS: u32 = 5;
pub const LIMIT_AGENT_VOL_MIN: u64 = 50;
pub const LIMIT_AGENT_VOL_MAX: u64 = 150;
pub const LIMIT_AGENT_MAX_OFFSET: u64 = 50;

// --- WhaleAgent ---
pub const WHALE_ACTION_PROB: f64 = 0.5;
pub const WHALE_ORDER_VOLUME: u64 = 500_000;
pub const WHALE_INITIAL_INVENTORY: i64 = 10_000_000;
pub const WHALE_PRICE_OFFSET_MIN: u64 = 500;
pub const WHALE_PRICE_OFFSET_MAX: u64 = 1000;
pub const CRAZY_WHALE: f64 = 0.05;

// FIXED: Added missing constants for the Market Maker agent.
// --- MarketMakerAgent ---
pub const MARKET_MAKER_ACTION_PROB: f64 = 0.95;
pub const MARKET_MAKER_SPREAD: u64 = 10; // e.g., 10 cents
pub const MARKET_MAKER_VOL_MIN: u64 = 100;
pub const MARKET_MAKER_VOL_MAX: u64 = 500;