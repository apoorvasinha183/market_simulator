# 🐋 WHALE SENTIMENT-RESPONSIVE UPGRADE PLAN

## **Objective**
Convert whale agent from absolute price offsets to percentage-based pricing with sentiment/momentum-driven institutional pressure.

## **Current State Analysis**
- Whale uses absolute price offsets (`WHALE_PRICE_OFFSET_MAX = 5000` = $50.00)
- Uses `Normal::new(0.0, std_dev)` for random offsets
- Places symmetric bids/asks: `mid ± offset`
- No sentiment/momentum consideration
- Working stable implementation

## **Target Behavior**
- **Bullish sentiment/momentum** → Raise bids (aggressive buying), normal asks → Upward price pressure
- **Bearish sentiment/momentum** → Lower asks (aggressive selling), normal bids → Downward price pressure
- **Percentage-based pricing** → Scales properly across different stock price levels

## **Implementation Phases**

### **Phase 1: Convert to Percentage-Based Pricing** ✅ COMPLETE
**Files:** `src/agents/config.rs`, `src/agents/whale_agent.rs`

**Config Changes:** ✅
```rust
// Replace absolute offsets
pub const WHALE_PRICE_OFFSET_MAX_PCT: f64 = 0.05; // 5% max offset from mid
pub const WHALE_PRICE_OFFSET_MIN_PCT: f64 = 0.005; // 0.5% min offset from mid
```

**Whale Logic Changes:** ✅
```rust
// OLD: offset = normal.sample(&mut rng).abs()
// NEW: 
let base_offset_pct = normal.sample(&mut rng).abs().max(min_offset_pct);
let offset = (current_mid_price as f64 * base_offset_pct).round() as u64;
```

**Test Result:** ✅ Compiles successfully, whale now uses percentage-based pricing

### **Phase 2: Add DashMap Dependency & Sentiment Tracking** ✅ COMPLETE
**Files:** `Cargo.toml`, `src/agents/whale_agent.rs`

**Add Dependency:** ✅ (Already present)
```toml
dashmap = "6.1.0"
```

**Struct Changes:** ✅
```rust
use dashmap::DashMap;

pub struct WhaleAgent {
    // ... existing fields ...
    sentiment_scores: Arc<DashMap<u64, f64>>,     // -1.0 to 1.0 per stock
    momentum_scores: Arc<DashMap<u64, f64>>,      // -1.0 to 1.0 per stock
}
```

**Constructor Updates:** ✅
```rust
sentiment_scores: Arc::new(DashMap::new()),
momentum_scores: Arc::new(DashMap::new()),
```

**Test Result:** ✅ Compiles successfully, sentiment tracking fields ready

### **Phase 3: Implement Directional Bias Formula**
**Files:** `src/agents/whale_agent.rs`

**New Functions:**
```rust
fn calculate_directional_bias(sentiment: f64, momentum: f64) -> f64 {
    let combined = (sentiment * 0.6) + (momentum * 0.4);
    combined.clamp(-1.0, 1.0)
}

fn apply_institutional_pressure(base_offset_pct: f64, bias: f64) -> (f64, f64) {
    let aggressiveness = 0.02; // 2% max adjustment
    
    if bias > 0.0 { // Bullish - raise bids, keep asks normal
        let bid_boost = bias * aggressiveness;
        let ask_penalty = bias * aggressiveness * 0.5;
        (base_offset_pct - bid_boost, base_offset_pct + ask_penalty)
    } else if bias < 0.0 { // Bearish - lower asks, keep bids normal
        let bid_penalty = bias.abs() * aggressiveness * 0.5;
        let ask_boost = bias.abs() * aggressiveness;
        (base_offset_pct + bid_penalty, base_offset_pct - ask_boost)
    } else { // Neutral
        (base_offset_pct, base_offset_pct)
    }
}
```

**Test:** Unit test the bias calculation with various sentiment/momentum values

### **Phase 4: Integrate Sentiment Data Source**
**Files:** `src/agents/whale_agent.rs`

**Event Handling (if available):**
```rust
// In event loop or similar
MarketEvent::SentimentUpdate { stock_id, score } => {
    self.sentiment_scores.insert(stock_id, score.clamp(-1.0, 1.0));
}
```

**Momentum Calculation (from trades):**
```rust
// Track price history and calculate momentum
// Insert into momentum_scores DashMap
```

**Test:** Verify sentiment/momentum data flows correctly

### **Phase 5: Apply Institutional Pressure to Order Placement**
**Files:** `src/agents/whale_agent.rs`

**Modified Order Loop:**
```rust
for _ in 0..WHALE_TAPER_ORDERS {
    let base_offset_pct = normal.sample(&mut rng).abs().max(WHALE_PRICE_OFFSET_MIN_PCT);
    
    // Get sentiment/momentum for this stock
    let sentiment = self.sentiment_scores.get(&stock_id).map(|v| *v).unwrap_or(0.0);
    let momentum = self.momentum_scores.get(&stock_id).map(|v| *v).unwrap_or(0.0);
    
    // Calculate directional bias
    let bias = Self::calculate_directional_bias(sentiment, momentum);
    let (bid_offset_pct, ask_offset_pct) = Self::apply_institutional_pressure(base_offset_pct, bias);
    
    // Apply percentage-based offsets
    let bid_offset = (current_mid_price as f64 * bid_offset_pct) as u64;
    let ask_offset = (current_mid_price as f64 * ask_offset_pct) as u64;
    
    // Place asymmetric orders
    let bid_px = current_mid_price.saturating_sub(bid_offset);
    let ask_px = current_mid_price.saturating_add(ask_offset);
    
    // Send orders...
}
```

**Test:** Verify asymmetric order placement creates institutional pressure

## **Configuration Parameters**
```rust
// Percentage-based pricing
pub const WHALE_PRICE_OFFSET_MAX_PCT: f64 = 0.05;  // 5% max
pub const WHALE_PRICE_OFFSET_MIN_PCT: f64 = 0.005; // 0.5% min

// Sentiment/momentum weighting
pub const WHALE_SENTIMENT_WEIGHT: f64 = 0.6;       // 60% sentiment
pub const WHALE_MOMENTUM_WEIGHT: f64 = 0.4;        // 40% momentum

// Institutional pressure strength
pub const WHALE_AGGRESSIVENESS_FACTOR: f64 = 0.02; // 2% max price adjustment
```

## **Risk Mitigation**
- Each phase is independently testable
- Can revert to previous phase if issues arise
- DashMap prevents deadlocks
- Percentage-based pricing scales across stock prices
- Bias calculations are clamped to prevent extreme behavior

## **Success Criteria**
- Whale places orders at percentage-based offsets
- Bullish sentiment creates upward price pressure
- Bearish sentiment creates downward price pressure
- No deadlocks or performance degradation
- Market remains stable with institutional pressure

## **Rollback Plan**
If any phase fails:
1. `git stash` current changes
2. Return to previous working phase
3. Debug issue in isolation
4. Resume from stable state

---
**Created:** Session continuation plan for whale sentiment upgrade
**Status:** Phase 1-3 Complete ✅, Phase 4-5 Partially Complete (need event handling)

## **Current Status Update**
- ✅ **Phase 1:** Percentage-based pricing implemented
- ✅ **Phase 2:** DashMap sentiment/momentum tracking added  
- ✅ **Phase 3:** Directional bias formula implemented and integrated
- 🔄 **Phase 4:** Sentiment data source - NEEDS event handling for live updates
- ✅ **Phase 5:** Institutional pressure applied to order placement

**Next Step:** Add event receiver to whale agent for live sentiment updates