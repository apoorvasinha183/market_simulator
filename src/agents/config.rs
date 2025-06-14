// src/agents/config.rs

//! A centralized place for tuning agent behavior parameters.

// --- General ---
pub const TICKS_UNTIL_ACTIVE: u32 = 5;
pub const MARGIN_CALL_THRESHOLD: i64 = -20_000;

// --- MarketMakerAgent ---
// The Market Maker's role is to provide a thick, stable book.
// These parameters are our baseline for liquidity.
pub const MM_INITIAL_INVENTORY: i64 = 100_000_000;
pub const MM_INITIAL_CENTER_PRICE: u64 = 15_000;
pub const MM_DESIRED_SPREAD: u64 = 25;
pub const MM_SKEW_FACTOR: f64 = 0.00001;
pub const MM_SEED_LEVELS: usize = 10;
pub const MM_SEED_DECAY: f64 = 0.90;
pub const MM_SEED_DEPTH_PCT: f64 = 0.002;
pub const MM_SEED_TICK_SPACING: u64 = 5;
pub const MM_UNSTICK_VOL_MIN: u64 = 5_000;
pub const MM_UNSTICK_VOL_MAX: u64 = 25_000;
pub const MM_QUOTE_VOL_MIN: u64 = 1_000;
pub const MM_QUOTE_VOL_MAX: u64 = 10_000;

// --- DumbAgent (Retail Market Orders) ---
// This ensemble now represents the full retail market, with occasional "burn" events.
pub const DUMB_AGENT_NUM_TRADERS: u32 = 50; // Increased population size
pub const DUMB_AGENT_ACTION_PROB: f64 = 0.3; // More active population
// Most retail flow is small "noise" trading.
pub const DUMB_AGENT_TYPICAL_VOL_MIN: u64 = 1;
pub const DUMB_AGENT_TYPICAL_VOL_MAX: u64 = 50;
// A "burn" event is rare (1% chance per trader) but represents a correlated,
// high-impact market order that can clear several levels of the book.
pub const DUMB_AGENT_LARGE_VOL_CHANCE: f64 = 0.001;
pub const DUMB_AGENT_LARGE_VOL_MIN: u64 = 75_00;
pub const DUMB_AGENT_LARGE_VOL_MAX: u64 = 750_000;

// --- DumbLimitAgent (Smarter Retail & Speculators) ---
// This ensemble now represents a more significant portion of the resting order book.
pub const LIMIT_AGENT_ACTION_PROB: f64 = 0.5;
// Their order sizes are now more substantial, able to absorb some of the "noise".
pub const LIMIT_AGENT_VOL_MIN: u64 = 500;
pub const LIMIT_AGENT_VOL_MAX: u64 = 5_000;
// The speculative offset remains large, representing diverse opinions on price.
pub const LIMIT_AGENT_MAX_OFFSET: u64 = 200; // $5.00 in cents
pub const LIMIT_AGENT_NUM_TRADERS: u32 = 200;
// The whales are here
pub const WHALE_INITIAL_INVENTORY: i64 = 50_000_000;
pub const WHALE_ACTION_PROB: f64 = 0.01; // Acts very infrequently (5% chance per tick)
pub const WHALE_ORDER_VOLUME: u64 = 1_000_000; // Places massive orders
pub const WHALE_PRICE_OFFSET_MAX: u64 = 1000;
pub const WHALE_PRICE_OFFSET_MIN: u64 = 500;
pub const CRAZY_WHALE: f64 = 0.01;
