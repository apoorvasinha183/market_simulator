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
pub const MM_DESIRED_SPREAD: u64 = 10; // $0.10

pub const MM_SKEW_FACTOR: f64 = 0.00001;

pub const MM_SEED_LEVELS: usize = 10;

pub const MM_SEED_DECAY: f64 = 0.90;

pub const MM_SEED_DEPTH_PCT: f64 = 0.01;

pub const MM_SEED_TICK_SPACING: u64 = 5;

pub const MM_UNSTICK_VOL_MIN: u64 = 5_000;

pub const MM_UNSTICK_VOL_MAX: u64 = 25_000;

pub const MM_QUOTE_VOL_MIN: u64 = 1_000;

pub const MM_QUOTE_VOL_MAX: u64 = 10_000;

// --- DumbAgent (Retail Market Orders) ---

// This ensemble now represents the full retail market, with occasional "burn" events.

pub const DUMB_AGENT_NUM_TRADERS: u32 = 50;

pub const DUMB_AGENT_ACTION_PROB: f64 = 0.3;

// Most retail flow is small "noise" trading.

pub const DUMB_AGENT_TYPICAL_VOL_MIN: u64 = 1;

pub const DUMB_AGENT_TYPICAL_VOL_MAX: u64 = 50;

// A "burn" event is rare but represents a correlated, high-impact market order.
// These volumes are large relative to typical retail, but not market-breaking.
pub const DUMB_AGENT_LARGE_VOL_CHANCE: f64 = 0.01; // 1% chance

pub const DUMB_AGENT_LARGE_VOL_MIN: u64 = 7_500;

pub const DUMB_AGENT_LARGE_VOL_MAX: u64 = 20_000;

// --- DumbLimitAgent (Smarter Retail & Speculators) ---

// This ensemble represents a smaller group of more sophisticated retail traders.

pub const LIMIT_AGENT_ACTION_PROB: f64 = 0.50;

// Their order sizes are more substantial, able to absorb some of the "noise".

pub const LIMIT_AGENT_VOL_MIN: u64 = 500;

pub const LIMIT_AGENT_VOL_MAX: u64 = 5_000;

// A smaller offset for placing limit orders, more realistic.
pub const LIMIT_AGENT_MAX_OFFSET: u64 = 100; // $1.00 in cents

pub const LIMIT_AGENT_NUM_TRADERS: u32 = 50;

// --- WhaleAgent ---

// The whale acts infrequently but with significant size.

pub const WHALE_INITIAL_INVENTORY: i64 = 50_000_000;

// Acts more frequently to have a noticeable impact on the simulation
pub const WHALE_ACTION_PROB: f64 = 0.05;

pub const WHALE_ORDER_VOLUME: u64 = 100_000_000; // Places massive orders

// Reduced price offset to be aggressive but not completely unrealistic.
pub const WHALE_PRICE_OFFSET_MAX: u64 = 2000; // $20.00
pub const WHALE_PRICE_OFFSET_MIN: u64 = 500; // $5.00

pub const CRAZY_WHALE: f64 = 0.01;

// --- Latency Simulation ---
// These values control how many events are processed before the public-facing
// market data (shadow book) is updated. Lower is faster/fresher.
// Note: These are currently set in `grpc_server.rs` not here.
pub const NORMAL_PROCESSING_LATENCY: usize = 1000; // Normal agents
pub const PREMIUM_PROCESSING_LATENCY: usize = 100; // Premium agents