// src/simulation/price_history_tracker.rs

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// A simple price point with timestamp and price
#[derive(Debug, Clone, serde::Serialize)]
pub struct PricePoint {
    pub timestamp: f64, // Unix timestamp in seconds (with milliseconds as decimal)
    pub price: f64,     // Price in dollars (not cents)
}

/// Tracks continuous price history for all stocks
/// This is separate from candle data and maintains a high-frequency time series
#[derive(Debug, Clone)]
pub struct PriceHistoryTracker {
    /// Stock ID -> VecDeque of price points
    /// We use VecDeque for efficient push_back and pop_front operations
    price_histories: Arc<RwLock<HashMap<u64, VecDeque<PricePoint>>>>,

    /// Maximum number of price points to keep per stock
    max_history_length: usize,

    /// Minimum time interval between price updates (in milliseconds)
    /// This prevents spam updates when prices change rapidly
    min_update_interval_ms: u64,

    /// Last update timestamp for each stock
    last_update_times: Arc<RwLock<HashMap<u64, u64>>>,
}

impl PriceHistoryTracker {
    /// Create a new price history tracker
    pub fn new(max_history_length: usize, min_update_interval_ms: u64) -> Self {
        Self {
            price_histories: Arc::new(RwLock::new(HashMap::new())),
            max_history_length,
            min_update_interval_ms,
            last_update_times: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create with default settings optimized for web interface
    pub fn new_default() -> Self {
        Self::new(
            5000, // Keep last 5000 price points per stock
            50,   // Update at most every 50ms (20 Hz)
        )
    }

    /// Add a new price point for a stock
    pub fn update_price(&self, stock_id: u64, price_cents: u64) {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // Check if enough time has passed since last update
        {
            let last_times = self.last_update_times.read().unwrap();
            if let Some(&last_time) = last_times.get(&stock_id) {
                if now_ms - last_time < self.min_update_interval_ms {
                    return; // Skip this update
                }
            }
        }

        // Update the last update time
        {
            let mut last_times = self.last_update_times.write().unwrap();
            last_times.insert(stock_id, now_ms);
        }

        let price_point = PricePoint {
            timestamp: now_ms as f64 / 1000.0, // Convert to seconds with decimal precision
            price: price_cents as f64 / 100.0, // Convert cents to dollars
        };

        let mut histories = self.price_histories.write().unwrap();
        let history = histories.entry(stock_id).or_insert_with(VecDeque::new);

        // Add new price point
        history.push_back(price_point);

        // Trim history if it's too long
        while history.len() > self.max_history_length {
            history.pop_front();
        }
    }

    /// Get the complete price history for a stock
    pub fn get_price_history(&self, stock_id: u64) -> Vec<PricePoint> {
        let histories = self.price_histories.read().unwrap();
        histories
            .get(&stock_id)
            .map(|deque| deque.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Get price history for a stock as [timestamp, price] arrays (for frontend)
    pub fn get_price_history_arrays(&self, stock_id: u64) -> Vec<[f64; 2]> {
        let histories = self.price_histories.read().unwrap();
        histories
            .get(&stock_id)
            .map(|deque| {
                deque
                    .iter()
                    .map(|point| [point.timestamp, point.price])
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get price histories for all stocks as [timestamp, price] arrays
    pub fn get_all_price_histories(&self) -> HashMap<u64, Vec<[f64; 2]>> {
        let histories = self.price_histories.read().unwrap();
        histories
            .iter()
            .map(|(&stock_id, deque)| {
                let arrays = deque
                    .iter()
                    .map(|point| [point.timestamp, point.price])
                    .collect();
                (stock_id, arrays)
            })
            .collect()
    }

    /// Get the latest price for a stock
    pub fn get_latest_price(&self, stock_id: u64) -> Option<f64> {
        let histories = self.price_histories.read().unwrap();
        histories
            .get(&stock_id)
            .and_then(|deque| deque.back())
            .map(|point| point.price)
    }

    /// Get recent price updates since a given timestamp
    /// This is useful for incremental updates to the frontend
    pub fn get_price_updates_since(&self, since_timestamp: f64) -> HashMap<u64, Vec<[f64; 2]>> {
        let histories = self.price_histories.read().unwrap();
        histories
            .iter()
            .filter_map(|(&stock_id, deque)| {
                let recent_points: Vec<[f64; 2]> = deque
                    .iter()
                    .filter(|point| point.timestamp > since_timestamp)
                    .map(|point| [point.timestamp, point.price])
                    .collect();

                if recent_points.is_empty() {
                    None
                } else {
                    Some((stock_id, recent_points))
                }
            })
            .collect()
    }

    /// Clear all price history (useful for testing)
    pub fn clear_all(&self) {
        let mut histories = self.price_histories.write().unwrap();
        histories.clear();
        let mut last_times = self.last_update_times.write().unwrap();
        last_times.clear();
    }

    /// Get statistics about the price history tracker
    pub fn get_stats(&self) -> PriceHistoryStats {
        let histories = self.price_histories.read().unwrap();
        let total_points: usize = histories.values().map(|deque| deque.len()).sum();
        let stocks_tracked = histories.len();

        PriceHistoryStats {
            stocks_tracked,
            total_points,
            max_history_length: self.max_history_length,
            min_update_interval_ms: self.min_update_interval_ms,
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct PriceHistoryStats {
    pub stocks_tracked: usize,
    pub total_points: usize,
    pub max_history_length: usize,
    pub min_update_interval_ms: u64,
}

/// Handle type for sharing the price history tracker
pub type PriceHistoryHandle = Arc<PriceHistoryTracker>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_price_history_basic() {
        let tracker = PriceHistoryTracker::new_default();

        // Add some price points
        tracker.update_price(1, 15000); // $150.00
        //add sleep so that we can test rate limiting
        thread::sleep(Duration::from_millis(100));
        tracker.update_price(1, 15050); // $150.50
        tracker.update_price(2, 30000); // $300.00

        let history1 = tracker.get_price_history_arrays(1);
        let history2 = tracker.get_price_history_arrays(2);

        assert_eq!(history1.len(), 2);
        assert_eq!(history2.len(), 1);
        assert_eq!(history1[0][1], 150.0);
        assert_eq!(history1[1][1], 150.5);
        assert_eq!(history2[0][1], 300.0);
    }

    #[test]
    fn test_rate_limiting() {
        let tracker = PriceHistoryTracker::new(1000, 100); // 100ms minimum interval

        // Add multiple price points rapidly
        tracker.update_price(1, 15000);
        tracker.update_price(1, 15010); // Should be ignored due to rate limiting
        tracker.update_price(1, 15020); // Should be ignored due to rate limiting

        let history = tracker.get_price_history_arrays(1);
        assert_eq!(history.len(), 1); // Only first update should be recorded

        // Wait and try again
        thread::sleep(Duration::from_millis(150));
        tracker.update_price(1, 15030); // Should be recorded now

        let history = tracker.get_price_history_arrays(1);
        assert_eq!(history.len(), 2);
    }

    #[test]
    fn test_history_trimming() {
        let tracker = PriceHistoryTracker::new(3, 0); // Max 3 points, no rate limiting

        // Add more points than the limit
        for i in 0..5 {
            tracker.update_price(1, 15000 + i * 10);
            thread::sleep(Duration::from_millis(1)); // Ensure different timestamps
        }

        let history = tracker.get_price_history_arrays(1);
        assert_eq!(history.len(), 3); // Should be trimmed to 3 points

        // Should contain the last 3 points
        assert_eq!(history[0][1], 150.20); // 15020 cents
        assert_eq!(history[1][1], 150.30); // 15030 cents
        assert_eq!(history[2][1], 150.40); // 15040 cents
    }
}
