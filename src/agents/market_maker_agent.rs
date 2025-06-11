use rand::Rng;
use super::agent_trait::{Agent, MarketView};
use crate::types::order::{OrderRequest, Side};

/// Hard guard-rails so quotes can never leave a sensible band
const MIN_PRICE: u64 = 1_00;        // $0.01  (cents)
const MAX_PRICE: u64 = 1_000_00;    // $10 000.00

#[inline]
fn clamp_price(p: i128) -> u64 {
    p.max(MIN_PRICE as i128)
     .min(MAX_PRICE as i128) as u64
}

pub struct MarketMakerAgent {
    pub id: usize,
    inventory: i64,
    desired_spread: u64,
    skew_factor: f64,
    initial_center_price: u64,
    ticks_until_active: u32,
    bootstrapped: bool,
}

impl MarketMakerAgent {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            inventory: 1_000_000,
            desired_spread: 25,
            skew_factor: 0.1,
            initial_center_price: 15_000, // $150.00 (cents)
            ticks_until_active: 5,
            bootstrapped: false,
        }
    }

    /// Build an initial depth ladder (10 levels either side, tapering volume).
    fn seed_liquidity(&self) -> Vec<OrderRequest> {
        let mut orders = Vec::with_capacity(20);
        let base_vol: u64 = 30_000;  // size at level-0
        let levels: u64  = 10;       // depth both sides

        for lvl in 0..levels {
            let vol = base_vol.saturating_sub(lvl * 2_000); // linear taper
            let bid_px = clamp_price(
                self.initial_center_price as i128
                    - (self.desired_spread / 2 + lvl) as i128,
            );
            let ask_px = clamp_price(
                self.initial_center_price as i128
                    + (self.desired_spread / 2 + lvl) as i128,
            );

            orders.push(OrderRequest::LimitOrder {
                agent_id: self.id,
                side: Side::Buy,
                price: bid_px,
                volume: vol,
            });
            orders.push(OrderRequest::LimitOrder {
                agent_id: self.id,
                side: Side::Sell,
                price: ask_px,
                volume: vol,
            });
        }
        orders
    }
}

impl Agent for MarketMakerAgent {
    fn decide_actions(&mut self, market_view: &MarketView) -> Vec<OrderRequest> {
        // Wait the configured latency ticks
        if self.ticks_until_active > 0 {
            self.ticks_until_active -= 1;
            return vec![];
        }

        // ---- bootstrap initial ladder once ----
        if !self.bootstrapped {
            self.bootstrapped = true;
            return self.seed_liquidity();
        }

        // -------- quote-update logic -----------
        let mut rng = rand::thread_rng();
        let best_bid = market_view.order_book.bids.keys().last().cloned();
        let best_ask = market_view.order_book.asks.keys().next().cloned();

        let center_price = match (best_bid, best_ask) {
            (Some(bid), Some(ask)) if ask > bid => {
                (((bid as u128 + ask as u128) / 2) as u64)
            }
            (None, Some(ask)) => ask.saturating_sub(self.desired_spread),
            (Some(bid), None) => bid.saturating_add(self.desired_spread),
            _ => return vec![], // empty or crossed → wait
        };

        let inventory_skew = (self.inventory as f64 * self.skew_factor) as i64;
        let our_center_price =
            clamp_price(center_price as i128 - inventory_skew as i128);

        let our_bid = clamp_price(
            our_center_price as i128 - (self.desired_spread / 2) as i128,
        );
        let our_ask = clamp_price(
            our_center_price as i128 + (self.desired_spread / 2) as i128,
        );

        if our_ask > our_bid {
            if best_ask.map_or(true, |ask| our_bid < ask)
                && best_bid.map_or(true, |bid| our_ask > bid)
            {
                let volume = rng.gen_range(50_000..=100_000);
                return vec![
                    OrderRequest::LimitOrder {
                        agent_id: self.id,
                        side: Side::Buy,
                        price: our_bid,
                        volume,
                    },
                    OrderRequest::LimitOrder {
                        agent_id: self.id,
                        side: Side::Sell,
                        price: our_ask,
                        volume,
                    },
                ];
            }
        }

        // Short-covering safety valve
        if self.inventory <= -20_000 {
            return vec![OrderRequest::MarketOrder {
                agent_id: self.id,
                side: Side::Buy,
                volume: (-self.inventory) as u64,
            }];
        }
        vec![]
    }

    // --- bookkeeping ---
    fn update_portfolio(&mut self, trade_volume: i64) {
        self.inventory = self.inventory.saturating_add(trade_volume);
        println!("MarketMaker {} new inventory: {}", self.id, self.inventory);
    }
    fn get_id(&self) -> usize { self.id }
    fn clone_agent(&self) -> Box<dyn Agent> {
        Box::new(MarketMakerAgent::new(self.id))
    }
}
