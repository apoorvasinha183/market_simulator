//! benches/order_book.rs
//! Run with:  cargo bench --bench order_book
//! HTML:      target/criterion/report/index.html

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

use market_simulator::{
    simulators::order_book::OrderBook,
    types::order::{Order, Side},
};
use rand::{Rng, SeedableRng, rngs::StdRng};
use std::hint::black_box;

// ────────────────────────────────────────────────────────────────────────────
//  Parameter grids
// ────────────────────────────────────────────────────────────────────────────
const BOOK_SIZES: &[usize] = &[50_000, 100_000, 500_000, 1_000_000];
const SWEEP_VOLUMES: &[u64] = &[25_000, 100_000, 250_000];

/// Build a fresh OrderBook with `n_orders` *sell* orders.
/// Prices cycle 100-109; volumes random 1-256.
fn setup_book(n_orders: usize) -> OrderBook {
    let mut rng = StdRng::seed_from_u64(42);
    let mut book = OrderBook::new();
    for i in 0..n_orders as u64 {
        let price = 100 + (i % 10); // 10 price levels
        let volume = rng.gen_range(1..=256) as u64;
        let mut order = Order {
            id: i,
            agent_id: (i % 10) as usize,
            side: Side::Sell,
            price,
            volume,
            filled: 0,
        };
        // Public insertion path
        let _ = book.process_limit_order(&mut order);
    }
    book
}

pub fn bench_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("order_matching_scaling");

    for &n in BOOK_SIZES {
        // Each n gets its own throughput baseline
        group.throughput(Throughput::Elements(n as u64));

        for &sweep in SWEEP_VOLUMES {
            let id = BenchmarkId::from_parameter(format!("book_{}_sweep_{}", n, sweep));

            group.bench_function(id, |b| {
                // rebuild a *fresh* OrderBook each iteration
                b.iter_batched(
                    || setup_book(n),
                    |mut book| {
                        let trades = book.process_market_order(black_box(999), Side::Buy, sweep);
                        black_box(trades);
                    },
                    BatchSize::LargeInput,
                );
            });
        }
    }
    group.finish();
}

criterion_group!(benches, bench_scaling);
criterion_main!(benches);
