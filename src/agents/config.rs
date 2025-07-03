// src/agents/config.rs

//! A centralized place for tuning agent behavior parameters.

// --- General ---

pub const TICKS_UNTIL_ACTIVE: u32 = 5;

// A more realistic margin call threshold for a market maker
pub const MARGIN_CALL_THRESHOLD: i64 = -2_000_000; // -$20,000

// --- MarketMakerAgent ---

// The Market Maker's role is to provide a thick, stable book.

// These parameters are our baseline for liquidity.

pub const MM_INITIAL_INVENTORY: i64 = 100_000_000;

pub const MM_INITIAL_CENTER_PRICE: u64 = 15_000; // $150.00

// A tighter spread for a more competitive market
pub const MM_DESIRED_SPREAD: u64 = 8; // $0.08

pub const MM_SKEW_FACTOR: f64 = 0.00001;

pub const MM_SEED_LEVELS: usize = 15;

pub const MM_SEED_DECAY: f64 = 0.85;

pub const MM_SEED_DEPTH_PCT: f64 = 0.03; // Increased initial liquidity

pub const MM_SEED_TICK_SPACING: u64 = 2;

pub const MM_UNSTICK_VOL_MIN: u64 = 1_000;

pub const MM_UNSTICK_VOL_MAX: u64 = 10_000;

pub const MM_QUOTE_VOL_MIN: u64 = 500;

pub const MM_QUOTE_VOL_MAX: u64 = 5_000;

// --- DumbAgent (Retail Market Orders) ---

// This ensemble now represents the full retail market, with occasional "burn" events.

pub const DUMB_AGENT_NUM_TRADERS: u32 = 100;

pub const DUMB_AGENT_ACTION_PROB: f64 = 0.5;

// Most retail flow is small "noise" trading.

pub const DUMB_AGENT_TYPICAL_VOL_MIN: u64 = 1;

pub const DUMB_AGENT_TYPICAL_VOL_MAX: u64 = 50; // Increased typical volume

// A "burn" event is rare but represents a correlated, high-impact market order.
// These volumes are large relative to typical retail, but not market-breaking.
pub const DUMB_AGENT_LARGE_VOL_CHANCE: f64 = 0.001; // 0.5% chance

pub const DUMB_AGENT_LARGE_VOL_MIN: u64 = 10_000;

pub const DUMB_AGENT_LARGE_VOL_MAX: u64 = 50_000;

// --- DumbLimitAgent (Smarter Retail & Speculators) ---

// This ensemble represents a smaller group of more sophisticated retail traders.

pub const LIMIT_AGENT_ACTION_PROB: f64 = 0.3;

// Their order sizes are more substantial, able to absorb some of the "noise".

pub const LIMIT_AGENT_VOL_MIN: u64 = 10000;

pub const LIMIT_AGENT_VOL_MAX: u64 = 100000;

// A smaller offset for placing limit orders, more realistic.
pub const LIMIT_AGENT_MAX_OFFSET: u64 = 50; // $0.50 in cents

pub const LIMIT_AGENT_NUM_TRADERS: u32 = 100;

// --- WhaleAgent ---

// The whale acts infrequently but with significant size.

pub const WHALE_INITIAL_INVENTORY: i64 = 500_000;

pub const WHALE_ACTION_PROB: f64 = 0.8;

pub const WHALE_ORDER_VOLUME: u64 = 5_000_000; // Places massive orders

// Reduced price offset to be aggressive but not completely unrealistic.
pub const WHALE_PRICE_OFFSET_MAX: u64 = 2000; // $20.00
pub const WHALE_PRICE_OFFSET_MIN: u64 = 100; // $1.00

pub const WHALE_REFRESH_THRESHOLD_BPS: u64 = 20; // 0.20%

pub const CRAZY_WHALE: f64 = 0.001;

// --- Latency Simulation ---
// These values control how many events are processed before the public-facing
// market data (shadow book) is updated. Lower is faster/fresher.
// Note: These are currently set in `grpc_server.rs` not here.
pub const NORMAL_PROCESSING_LATENCY: usize = 500; // Normal agents
pub const PREMIUM_PROCESSING_LATENCY: usize = 50; // Premium agents
