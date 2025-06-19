// src/sentiment.rs

use once_cell::sync::OnceCell;
use parking_lot::RwLock;
use rand::Rng;
use std::{collections::HashMap, thread, time::Duration};

/// What each stock’s sentiment looks like right now.
static SENTIMENT: OnceCell<RwLock<HashMap<u64, f64>>> = OnceCell::new();

pub struct SentimentConfig {
    pub tick_interval: Duration,
    pub spike_prob: f64,
    pub half_life: Duration,
}

pub fn init(stock_ids: Vec<u64>, cfg: SentimentConfig) {
    // initialize the shared map (once)
    let table = SENTIMENT.get_or_init(|| {
        let mut m = HashMap::new();
        for &id in &stock_ids {
            m.insert(id, 0.0);
        }
        RwLock::new(m)
    });

    // but only spawn *one* thread
    static SPAWNED: OnceCell<()> = OnceCell::new();
    if SPAWNED.set(()).is_err() {
        return; // already spawned
    }

    // decay factor per tick: at t = half_life → v_new = v_old * 2^(-1) = 0.5
    let decay = 2f64.powf(-cfg.tick_interval.as_secs_f64() / cfg.half_life.as_secs_f64());

    thread::spawn(move || {
        let mut rng = rand::thread_rng();
        loop {
            // wait one tick before touching the table
            thread::sleep(cfg.tick_interval);

            let mut map = table.write();
            for &id in &stock_ids {
                let v = map.get_mut(&id).unwrap();
                if rng.gen_bool(cfg.spike_prob) {
                    *v = rng.gen_range(-1.0..=1.0);
                } else {
                    *v *= decay;
                }
                // ensure we stay within [-1, 1]
                if *v > 1.0 {
                    *v = 1.0
                } else if *v < -1.0 {
                    *v = -1.0
                }
            }
        }
    });
}

/// Agents call this to get the latest sentiment (≈ instantaneous).
pub fn get(stock_id: u64) -> f64 {
    let table = SENTIMENT
        .get()
        .expect("sentiment_engine::init() must be called first");
    *table.read().get(&stock_id).unwrap_or(&0.0)
}
// ──────────────────────────────────────────────────────────────────────────────
//  Unit tests for sentiment engine
// ──────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    // A helper config with a very slow decay and no spontaneous spikes.
    fn cfg() -> SentimentConfig {
        SentimentConfig {
            tick_interval: Duration::from_millis(10),
            spike_prob: 0.0,
            half_life: Duration::from_secs(1),
        }
    }
    fn make_cfg(spike_prob: f64, tick_ms: u64, half_ms: u64) -> SentimentConfig {
        SentimentConfig {
            tick_interval: Duration::from_millis(tick_ms),
            spike_prob,
            half_life: Duration::from_millis(half_ms),
        }
    }

    #[test]
    fn init_is_idempotent() {
        // calling twice must not panic or spawn extra threads
        let ids = vec![1, 2, 3];
        init(ids.clone(), cfg());
        init(ids, cfg());
    }

    #[test]
    fn direct_write_and_get() {
        // bring up the engine
        let ids = vec![42];
        init(ids, cfg());

        // manually overwrite the sentiment table
        let table = SENTIMENT.get().unwrap();
        {
            let mut map = table.write();
            map.insert(42, 0.73);
        }

        // get must return exactly what we wrote
        assert_eq!(get(42), 0.73);
    }

    #[test]
    fn unknown_ticker_returns_zero() {
        let _ = init(vec![100], cfg());
        // 9999 was never inserted
        assert_eq!(get(9999), 0.0);
    }
    #[test]
    fn decay_factor_in_() {
        let cfg = make_cfg(0.0, 10, 100);
        let decay = 2f64.powf(-cfg.tick_interval.as_secs_f64() / cfg.half_life.as_secs_f64());
        assert!(
            decay > 0.0 && decay < 1.0,
            "decay should be in (0,1), got {}",
            decay
        );
    }

    #[test]
    fn spikes_never_escape_bounds() {
        let ids = vec![1u64, 2, 3];
        // 100% spike every tick, no decay
        init(ids.clone(), make_cfg(1.0, 10, 1));
        // give it a couple of ticks
        sleep(Duration::from_millis(35));
        for &id in &ids {
            let v = get(id);
            assert!(v >= -1.0 && v <= 1.0, "spike out of [-1,1]: {}", v);
        }
    }

    #[test]
    fn decay_calculation_is_correct() {
        let tick = 50;
        let half = 100;
        let cfg = make_cfg(0.0, tick, half);

        let decay = 2f64.powf(-cfg.tick_interval.as_secs_f64() / cfg.half_life.as_secs_f64());
        let expected = 2f64.powf(-0.5); // tick/half = 50/100 = 0.5

        assert!((decay - expected).abs() < 1e-10);
        assert_eq!(decay, 2f64.powf(-0.5)); // Should be ~0.7071
    }
    #[test]
    fn long_run_stays_bounded() {
        let ids = vec![7u64];
        // 50% chance spike, moderate decay
        init(ids.clone(), make_cfg(0.5, 20, 200));
        // run for a few hundred ms
        sleep(Duration::from_millis(500));
        let v = get(7);
        assert!(v >= -1.0 && v <= 1.0, "long‐run value out of [-1,1]: {}", v);
    }
}
