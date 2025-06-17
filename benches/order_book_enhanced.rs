//! benches/order_book_enhanced.rs
//! Run with:  cargo bench --bench order_book_enhanced
//! HTML:      target/criterion/report/index.html

use criterion::{
    BatchSize, BenchmarkId, Criterion, Throughput, 
    criterion_group, criterion_main
};
use market_simulator::{
    simulators::order_book::OrderBook,
    types::{Order, Side},
};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::hint::black_box;

// ──────────────────────────────────────────────────────────────────────────────
//  Parameter grids
// ──────────────────────────────────────────────────────────────────────────────

const BOOK_SIZES: &[usize] = &[10_000, 50_000, 100_000, 500_000, 1_000_000, 2_000_000];
const SWEEP_VOLUMES: &[u64] = &[1_000, 25_000, 100_000, 250_000, 1_000_000];
const PRICE_LEVEL_COUNTS: &[u64] = &[5, 10, 50, 100];

// ──────────────────────────────────────────────────────────────────────────────
//  Setup functions
// ──────────────────────────────────────────────────────────────────────────────

/// Build a fresh OrderBook with configurable parameters
fn setup_book_with_params(
    n_orders: usize,
    price_levels: u64,
    side: Side,
    base_price: u64,
    seed: u64,
) -> OrderBook {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut book = OrderBook::new();

    for i in 0..n_orders as u64 {
        let price = match side {
            Side::Sell => base_price + (i % price_levels),
            Side::Buy => base_price - (i % price_levels),
        };
        let volume = rng.gen_range(1..=256);

        let mut order = Order {
            id: i,
            agent_id: (i % 10) as usize,
            stock_id: 0,
            side,
            price,
            volume,
            filled: 0,
        };

        let _ = book.process_limit_order(&mut order);
    }

    book
}

/// Setup a sell-side book (original behavior)
fn setup_sell_book(n_orders: usize) -> OrderBook {
    setup_book_with_params(n_orders, 10, Side::Sell, 100, 42)
}

/// Setup a buy-side book
fn setup_buy_book(n_orders: usize) -> OrderBook {
    setup_book_with_params(n_orders, 10, Side::Buy, 100, 42)
}

/// Setup a mixed book with both buy and sell orders
fn setup_mixed_book(n_orders: usize) -> OrderBook {
    let mut rng = StdRng::seed_from_u64(42);
    let mut book = OrderBook::new();

    for i in 0..n_orders as u64 {
        let side = if i % 2 == 0 { Side::Buy } else { Side::Sell };
        let base_price = 100;
        let price = match side {
            Side::Buy => base_price - (i % 10),
            Side::Sell => base_price + (i % 10),
        };
        let volume = rng.gen_range(1..=256);

        let mut order = Order {
            id: i,
            agent_id: (i % 10) as usize,
            stock_id: 0,
            side,
            price,
            volume,
            filled: 0,
        };

        let _ = book.process_limit_order(&mut order);
    }

    book
}

// ──────────────────────────────────────────────────────────────────────────────
//  Benchmark functions
// ──────────────────────────────────────────────────────────────────────────────

pub fn bench_market_order_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("market_order_scaling");

    for &n in BOOK_SIZES {
        group.throughput(Throughput::Elements(n as u64));

        for &sweep in SWEEP_VOLUMES {
            // Skip combinations where sweep > book size to avoid empty results
            if sweep > (n as u64 * 256) { continue; }

            let id = BenchmarkId::from_parameter(format!("sell_book_{}_sweep_{}", n, sweep));
            group.bench_function(id, |b| {
                b.iter_batched(
                    || setup_sell_book(n),
                    |mut book| {
                        let trades = book.process_market_order(
                            black_box(999),
                            Side::Buy,
                            sweep,
                        );
                        black_box(trades);
                    },
                    BatchSize::LargeInput,
                )
            });
        }
    }

    group.finish();
}

pub fn bench_limit_order_insertion(c: &mut Criterion) {
    let mut group = c.benchmark_group("limit_order_insertion");

    for &n in &[10_000, 50_000, 100_000, 500_000] {
        group.throughput(Throughput::Elements(1000)); // 1000 insertions per iteration

        let id = BenchmarkId::from_parameter(format!("book_size_{}", n));
        group.bench_function(id, |b| {
            b.iter_batched(
                || {
                    let book = setup_sell_book(n);
                    let mut rng = StdRng::seed_from_u64(123);
                    let orders: Vec<Order> = (0..1000).map(|i| {
                        Order {
                            id: (n as u64) + i,
                            agent_id: (i % 10) as usize,
                            stock_id: 0,
                            side: Side::Sell,
                            price: 110 + (i % 20) as u64, // New price levels
                            volume: rng.gen_range(1..=100),
                            filled: 0,
                        }
                    }).collect();
                    (book, orders)
                },
                |(mut book, mut orders)| {
                    for order in &mut orders {
                        let result = book.process_limit_order(black_box(order));
                        black_box(result);
                    }
                },
                BatchSize::LargeInput,
            )
        });
    }

    group.finish();
}

pub fn bench_mixed_book_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_book_operations");

    for &n in &[50_000, 100_000, 500_000] {
        group.throughput(Throughput::Elements(n as u64));

        for &sweep in &[25_000, 100_000, 250_000] {
            if sweep > (n as u64 * 128) { continue; } // Mixed book has less depth per side

            // Test both buy and sell market orders against mixed book
            for side in [Side::Buy, Side::Sell] {
                let side_str = match side { Side::Buy => "buy", Side::Sell => "sell" };
                let id = BenchmarkId::from_parameter(
                    format!("mixed_book_{}_{}_{}", n, side_str, sweep)
                );
                
                group.bench_function(id, |b| {
                    b.iter_batched(
                        || setup_mixed_book(n),
                        |mut book| {
                            let trades = book.process_market_order(
                                black_box(999),
                                side,
                                sweep,
                            );
                            black_box(trades);
                        },
                        BatchSize::LargeInput,
                    )
                });
            }
        }
    }

    group.finish();
}

pub fn bench_price_level_impact(c: &mut Criterion) {
    let mut group = c.benchmark_group("price_level_scaling");
    
    const FIXED_ORDERS: usize = 100_000;
    const FIXED_SWEEP: u64 = 50_000;

    for &levels in PRICE_LEVEL_COUNTS {
        group.throughput(Throughput::Elements(FIXED_ORDERS as u64));

        let id = BenchmarkId::from_parameter(format!("levels_{}", levels));
        group.bench_function(id, |b| {
            b.iter_batched(
                || setup_book_with_params(FIXED_ORDERS, levels, Side::Sell, 100, 42),
                |mut book| {
                    let trades = book.process_market_order(
                        black_box(999),
                        Side::Buy,
                        FIXED_SWEEP,
                    );
                    black_box(trades);
                },
                BatchSize::LargeInput,
            )
        });
    }

    group.finish();
}

pub fn bench_order_cancellation(c: &mut Criterion) {
    let mut group = c.benchmark_group("order_cancellation");

    for &n in &[10_000, 50_000, 100_000] {
        group.throughput(Throughput::Elements(1000)); // 1000 cancellations per iteration

        let id = BenchmarkId::from_parameter(format!("book_size_{}", n));
        group.bench_function(id, |b| {
            b.iter_batched(
                || {
                    let book = setup_sell_book(n);
                    // Collect some order IDs that we know exist (first 1000 orders)
                    let cancel_requests: Vec<(u64, usize)> = (0..1000)
                        .map(|i| (i as u64, (i % 10) as usize)) // (order_id, agent_id)
                        .collect();
                    (book, cancel_requests)
                },
                |(mut book, cancel_requests)| {
                    for &(order_id, agent_id) in &cancel_requests {
                        // Match your clearinghouse API: cancel_order(order_id, agent_id) -> bool
                        let result = book.cancel_order(black_box(order_id), black_box(agent_id));
                        black_box(result);
                    }
                },
                BatchSize::LargeInput,
            )
        });
    }

    group.finish();
}

pub fn bench_short_covering_scenarios(c: &mut Criterion) {
    let mut group = c.benchmark_group("short_covering");

    // Simulate different short covering scenarios
    let scenarios = [
        ("light_covering", 10_000),    // Small short cover
        ("medium_covering", 50_000),   // Medium short cover  
        ("heavy_covering", 200_000),   // Large short cover (force multiple levels)
    ];

    const BOOK_SIZE: usize = 100_000;

    for (scenario_name, cover_volume) in scenarios {
        let id = BenchmarkId::from_parameter(scenario_name);
        group.bench_function(id, |b| {
            b.iter_batched(
                || setup_sell_book(BOOK_SIZE), // Deep sell book for shorts to cover against
                |mut book| {
                    // Short covering = forced market buy orders
                    let trades = book.process_market_order(
                        black_box(888), // Short covering agent
                        Side::Buy,      // Always buying to cover shorts
                        cover_volume,
                    );
                    black_box(trades);
                },
                BatchSize::LargeInput,
            )
        });
    }

    group.finish();
}

pub fn bench_negative_inventory_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("negative_inventory_simulation");

    // Simulate agents with negative inventory being forced to cover
    const BOOK_SIZE: usize = 50_000;
    
    // Different patterns of short covering frequency
    let patterns = [
        ("frequent_small", vec![5_000, 5_000, 5_000, 5_000]), // 4 small covers
        ("infrequent_large", vec![50_000]),                   // 1 large cover
        ("mixed_pattern", vec![10_000, 30_000, 5_000]),       // Mixed sizes
    ];

    for (pattern_name, cover_sequence) in patterns {
        let id = BenchmarkId::from_parameter(pattern_name);
        group.bench_function(id, |b| {
            b.iter_batched(
                || setup_sell_book(BOOK_SIZE),
                |mut book| {
                    // Execute sequence of short covers
                    for (i, &volume) in cover_sequence.iter().enumerate() {
                        let trades = book.process_market_order(
                            black_box(800 + i), // Different agent IDs for each cover
                            Side::Buy,
                            volume,
                        );
                        black_box(trades);
                    }
                },
                BatchSize::LargeInput,
            )
        });
    }

    group.finish();
}

// ──────────────────────────────────────────────────────────────────────────────
//  Criterion configuration
// ──────────────────────────────────────────────────────────────────────────────

// ──────────────────────────────────────────────────────────────────────────────
//  Criterion configuration
// ──────────────────────────────────────────────────────────────────────────────

criterion_group!(benches, 
    bench_market_order_scaling,
    bench_limit_order_insertion,
    bench_mixed_book_operations,
    bench_price_level_impact,
    bench_order_cancellation,
    bench_short_covering_scenarios,
    bench_negative_inventory_patterns
);
criterion_main!(benches);