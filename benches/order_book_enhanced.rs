//! benches/order_book_enhanced.rs
//! Run with:  cargo bench --bench order_book_enhanced
//! HTML:      target/criterion/report/index.html
//! CSV Data:  benchmark_results/ directory

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group};
use market_simulator::{
    simulators::order_book::OrderBook,
    types::{Order, Side},
};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::fs::{File, create_dir_all};
use std::hint::black_box;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::Mutex;
use std::time::Instant;

// ──────────────────────────────────────────────────────────────────────────────
//  CSV Export Utilities
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct BenchmarkResult {
    benchmark_group: String,
    test_name: String,
    parameter: String,
    book_size: Option<usize>,
    sweep_volume: Option<u64>,
    price_levels: Option<u64>,
    side: Option<String>,
    mean_time_ns: f64,
    std_dev_ns: f64,
    throughput_elements_per_sec: Option<f64>,
    sample_count: usize,
}

impl BenchmarkResult {
    fn to_csv_row(&self) -> String {
        format!(
            "{},{},{},{},{},{},{},{:.2},{:.2},{},{}\n",
            self.benchmark_group,
            self.test_name,
            self.parameter,
            self.book_size.map_or("".to_string(), |v| v.to_string()),
            self.sweep_volume.map_or("".to_string(), |v| v.to_string()),
            self.price_levels.map_or("".to_string(), |v| v.to_string()),
            self.side.as_ref().map_or("", |s| s),
            self.mean_time_ns,
            self.std_dev_ns,
            self.throughput_elements_per_sec
                .map_or("".to_string(), |v| format!("{:.2}", v)),
            self.sample_count
        )
    }
}

struct CsvExporter {
    results: Vec<BenchmarkResult>,
}

impl CsvExporter {
    fn new() -> Self {
        Self {
            results: Vec::new(),
        }
    }

    fn add_result(&mut self, result: BenchmarkResult) {
        self.results.push(result);
    }

    fn export_to_file(&self, filename: &str) -> std::io::Result<()> {
        create_dir_all("benchmark_results")?;
        let path = Path::new("benchmark_results").join(filename);
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        // Write header
        writeln!(
            writer,
            "benchmark_group,test_name,parameter,book_size,sweep_volume,price_levels,side,mean_time_ns,std_dev_ns,throughput_elements_per_sec,sample_count"
        )?;

        // Write data
        for result in &self.results {
            write!(writer, "{}", result.to_csv_row())?;
        }

        writer.flush()?;
        Ok(())
    }
}

// Thread-safe global CSV exporter
lazy_static::lazy_static! {
    static ref CSV_EXPORTER: Mutex<CsvExporter> = Mutex::new(CsvExporter::new());
}

fn add_result_to_exporter(result: BenchmarkResult) {
    if let Ok(mut exporter) = CSV_EXPORTER.lock() {
        exporter.add_result(result);
    }
}

fn export_all_results() {
    if let Ok(exporter) = CSV_EXPORTER.lock() {
        // Export comprehensive results
        if let Err(e) = exporter.export_to_file("comprehensive_benchmark_results.csv") {
            eprintln!("Failed to export comprehensive results: {}", e);
        } else {
            println!(
                "✓ Exported comprehensive results to benchmark_results/comprehensive_benchmark_results.csv"
            );
        }

        // Export grouped results for easier analysis
        let groups = [
            "market_order_scaling",
            "limit_order_insertion",
            "mixed_book_operations",
            "price_level_scaling",
            "order_cancellation",
            "short_covering",
            "negative_inventory_simulation",
        ];

        for group in &groups {
            let group_results: Vec<_> = exporter
                .results
                .iter()
                .filter(|r| r.benchmark_group == *group)
                .cloned()
                .collect();

            if !group_results.is_empty() {
                let filename = format!("{}_results.csv", group);
                let mut group_exporter = CsvExporter::new();
                for result in group_results {
                    group_exporter.add_result(result);
                }

                if let Err(e) = group_exporter.export_to_file(&filename) {
                    eprintln!("Failed to export {} results: {}", group, e);
                } else {
                    println!(
                        "✓ Exported {} results to benchmark_results/{}",
                        group, filename
                    );
                }
            }
        }

        println!(
            "\n📊 All benchmark results exported to CSV files in benchmark_results/ directory"
        );
        println!("   Ready for analysis in Jupyter notebook!");
    }
}

// Custom measurement function to capture timing data
fn measure_and_record<F>(
    benchmark_group: &str,
    test_name: &str,
    parameter: &str,
    book_size: Option<usize>,
    sweep_volume: Option<u64>,
    price_levels: Option<u64>,
    side: Option<&str>,
    setup_fn: F,
    iterations: usize,
) where
    F: Fn() -> Box<dyn FnMut()>,
{
    let mut times = Vec::new();

    for _ in 0..iterations {
        let mut bench_fn = setup_fn();
        let start = Instant::now();
        bench_fn();
        let duration = start.elapsed();
        times.push(duration.as_nanos() as f64);
    }

    let mean_time = times.iter().sum::<f64>() / times.len() as f64;
    let variance = times.iter().map(|&t| (t - mean_time).powi(2)).sum::<f64>() / times.len() as f64;
    let std_dev = variance.sqrt();

    let throughput = book_size.map(|size| size as f64 / (mean_time / 1_000_000_000.0));

    let result = BenchmarkResult {
        benchmark_group: benchmark_group.to_string(),
        test_name: test_name.to_string(),
        parameter: parameter.to_string(),
        book_size,
        sweep_volume,
        price_levels,
        side: side.map(|s| s.to_string()),
        mean_time_ns: mean_time,
        std_dev_ns: std_dev,
        throughput_elements_per_sec: throughput,
        sample_count: times.len(),
    };

    add_result_to_exporter(result);
}

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
// Allow dead code
#[allow(dead_code)]
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
//  Enhanced Benchmark functions with CSV export
// ──────────────────────────────────────────────────────────────────────────────

pub fn bench_market_order_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("market_order_scaling");

    for &n in BOOK_SIZES {
        group.throughput(Throughput::Elements(n as u64));

        for &sweep in SWEEP_VOLUMES {
            // Skip combinations where sweep > book size to avoid empty results
            if sweep > (n as u64 * 256) {
                continue;
            }

            let id = BenchmarkId::from_parameter(format!("sell_book_{}_sweep_{}", n, sweep));
            let param_str = format!("sell_book_{}_sweep_{}", n, sweep);

            group.bench_function(id, |b| {
                b.iter_batched(
                    || setup_sell_book(n),
                    |mut book| {
                        let trades = book.process_market_order(black_box(999), Side::Buy, sweep);
                        black_box(trades);
                    },
                    BatchSize::LargeInput,
                )
            });

            // Also capture data for CSV export
            measure_and_record(
                "market_order_scaling",
                "sell_book_market_order",
                &param_str,
                Some(n),
                Some(sweep),
                None,
                Some("Buy"),
                || {
                    let mut book = setup_sell_book(n);
                    Box::new(move || {
                        let trades = book.process_market_order(999, Side::Buy, sweep);
                        black_box(trades);
                    })
                },
                100, // Number of iterations for CSV measurement
            );
        }
    }

    group.finish();
}

pub fn bench_limit_order_insertion(c: &mut Criterion) {
    let mut group = c.benchmark_group("limit_order_insertion");

    for &n in &[10_000, 50_000, 100_000, 500_000] {
        group.throughput(Throughput::Elements(1000)); // 1000 insertions per iteration

        let id = BenchmarkId::from_parameter(format!("book_size_{}", n));
        let param_str = format!("book_size_{}", n);

        group.bench_function(id, |b| {
            b.iter_batched(
                || {
                    let book = setup_sell_book(n);
                    let mut rng = StdRng::seed_from_u64(123);
                    let orders: Vec<Order> = (0..1000)
                        .map(|i| {
                            Order {
                                id: (n as u64) + i,
                                agent_id: (i % 10) as usize,
                                stock_id: 0,
                                side: Side::Sell,
                                price: 110 + (i % 20) as u64, // New price levels
                                volume: rng.gen_range(1..=100),
                                filled: 0,
                            }
                        })
                        .collect();
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

        // CSV measurement
        measure_and_record(
            "limit_order_insertion",
            "batch_insert",
            &param_str,
            Some(n),
            None,
            None,
            Some("Sell"),
            || {
                let book = setup_sell_book(n);
                let mut rng = StdRng::seed_from_u64(123);
                let orders: Vec<Order> = (0..1000)
                    .map(|i| Order {
                        id: (n as u64) + i,
                        agent_id: (i % 10) as usize,
                        stock_id: 0,
                        side: Side::Sell,
                        price: 110 + (i % 20) as u64,
                        volume: rng.gen_range(1..=100),
                        filled: 0,
                    })
                    .collect();

                Box::new(move || {
                    let mut book_copy = book.clone();
                    let mut orders_copy = orders.clone();
                    for order in &mut orders_copy {
                        let result = book_copy.process_limit_order(order);
                        black_box(result);
                    }
                })
            },
            50,
        );
    }

    group.finish();
}

pub fn bench_mixed_book_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_book_operations");

    for &n in &[50_000, 100_000, 500_000] {
        group.throughput(Throughput::Elements(n as u64));

        for &sweep in &[25_000, 100_000, 250_000] {
            if sweep > (n as u64 * 128) {
                continue;
            }

            for side in [Side::Buy, Side::Sell] {
                let side_str = match side {
                    Side::Buy => "buy",
                    Side::Sell => "sell",
                };
                let id =
                    BenchmarkId::from_parameter(format!("mixed_book_{}_{}_{}", n, side_str, sweep));
                let param_str = format!("mixed_book_{}_{}_{}", n, side_str, sweep);

                group.bench_function(id, |b| {
                    b.iter_batched(
                        || setup_mixed_book(n),
                        |mut book| {
                            let trades = book.process_market_order(black_box(999), side, sweep);
                            black_box(trades);
                        },
                        BatchSize::LargeInput,
                    )
                });

                // CSV measurement
                measure_and_record(
                    "mixed_book_operations",
                    "mixed_market_order",
                    &param_str,
                    Some(n),
                    Some(sweep),
                    None,
                    Some(side_str),
                    || {
                        let mut book = setup_mixed_book(n);
                        Box::new(move || {
                            let trades = book.process_market_order(999, side, sweep);
                            black_box(trades);
                        })
                    },
                    50,
                );
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
        let param_str = format!("levels_{}", levels);

        group.bench_function(id, |b| {
            b.iter_batched(
                || setup_book_with_params(FIXED_ORDERS, levels, Side::Sell, 100, 42),
                |mut book| {
                    let trades = book.process_market_order(black_box(999), Side::Buy, FIXED_SWEEP);
                    black_box(trades);
                },
                BatchSize::LargeInput,
            )
        });

        // CSV measurement
        measure_and_record(
            "price_level_scaling",
            "price_level_impact",
            &param_str,
            Some(FIXED_ORDERS),
            Some(FIXED_SWEEP),
            Some(levels),
            Some("Buy"),
            || {
                let mut book = setup_book_with_params(FIXED_ORDERS, levels, Side::Sell, 100, 42);
                Box::new(move || {
                    let trades = book.process_market_order(999, Side::Buy, FIXED_SWEEP);
                    black_box(trades);
                })
            },
            50,
        );
    }

    group.finish();
}

pub fn bench_order_cancellation(c: &mut Criterion) {
    let mut group = c.benchmark_group("order_cancellation");

    for &n in &[10_000, 50_000, 100_000] {
        group.throughput(Throughput::Elements(1000)); // 1000 cancellations per iteration

        let id = BenchmarkId::from_parameter(format!("book_size_{}", n));
        let param_str = format!("book_size_{}", n);

        group.bench_function(id, |b| {
            b.iter_batched(
                || {
                    let book = setup_sell_book(n);
                    let cancel_requests: Vec<(u64, usize)> =
                        (0..1000).map(|i| (i as u64, (i % 10) as usize)).collect();
                    (book, cancel_requests)
                },
                |(mut book, cancel_requests)| {
                    for &(order_id, agent_id) in &cancel_requests {
                        let result = book.cancel_order(black_box(order_id), black_box(agent_id));
                        black_box(result);
                    }
                },
                BatchSize::LargeInput,
            )
        });

        // CSV measurement
        measure_and_record(
            "order_cancellation",
            "batch_cancel",
            &param_str,
            Some(n),
            None,
            None,
            None,
            || {
                let book = setup_sell_book(n);
                let cancel_requests: Vec<(u64, usize)> =
                    (0..1000).map(|i| (i as u64, (i % 10) as usize)).collect();

                Box::new(move || {
                    let mut book_copy = book.clone();
                    for &(order_id, agent_id) in &cancel_requests {
                        let result = book_copy.cancel_order(order_id, agent_id);
                        black_box(result);
                    }
                })
            },
            50,
        );
    }

    group.finish();
}

pub fn bench_short_covering_scenarios(c: &mut Criterion) {
    let mut group = c.benchmark_group("short_covering");

    let scenarios = [
        ("light_covering", 10_000),
        ("medium_covering", 50_000),
        ("heavy_covering", 200_000),
    ];

    const BOOK_SIZE: usize = 100_000;

    for (scenario_name, cover_volume) in scenarios {
        let id = BenchmarkId::from_parameter(scenario_name);

        group.bench_function(id, |b| {
            b.iter_batched(
                || setup_sell_book(BOOK_SIZE),
                |mut book| {
                    let trades = book.process_market_order(black_box(888), Side::Buy, cover_volume);
                    black_box(trades);
                },
                BatchSize::LargeInput,
            )
        });

        // CSV measurement
        measure_and_record(
            "short_covering",
            "short_cover_scenario",
            scenario_name,
            Some(BOOK_SIZE),
            Some(cover_volume),
            None,
            Some("Buy"),
            || {
                let mut book = setup_sell_book(BOOK_SIZE);
                Box::new(move || {
                    let trades = book.process_market_order(888, Side::Buy, cover_volume);
                    black_box(trades);
                })
            },
            50,
        );
    }

    group.finish();
}

pub fn bench_negative_inventory_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("negative_inventory_simulation");

    const BOOK_SIZE: usize = 50_000;

    let patterns = [
        ("frequent_small", vec![5_000, 5_000, 5_000, 5_000]),
        ("infrequent_large", vec![50_000]),
        ("mixed_pattern", vec![10_000, 30_000, 5_000]),
    ];

    for (pattern_name, cover_sequence) in patterns {
        let id = BenchmarkId::from_parameter(pattern_name);

        group.bench_function(id, |b| {
            b.iter_batched(
                || setup_sell_book(BOOK_SIZE),
                |mut book| {
                    for (i, &volume) in cover_sequence.iter().enumerate() {
                        let trades =
                            book.process_market_order(black_box(800 + i), Side::Buy, volume);
                        black_box(trades);
                    }
                },
                BatchSize::LargeInput,
            )
        });

        // CSV measurement
        let total_volume: u64 = cover_sequence.iter().sum();
        measure_and_record(
            "negative_inventory_simulation",
            "inventory_pattern",
            pattern_name,
            Some(BOOK_SIZE),
            Some(total_volume),
            None,
            Some("Buy"),
            || {
                let cover_seq = cover_sequence.clone();
                Box::new(move || {
                    let mut book = setup_sell_book(BOOK_SIZE);
                    for (i, &volume) in cover_seq.iter().enumerate() {
                        let trades = book.process_market_order(800 + i, Side::Buy, volume);
                        black_box(trades);
                    }
                })
            },
            50,
        );
    }

    group.finish();
}

// ──────────────────────────────────────────────────────────────────────────────
//  Main benchmark entry point
// ──────────────────────────────────────────────────────────────────────────────
// run dead code
#[allow(dead_code)]
fn run_benchmarks() {
    let mut criterion = Criterion::default();

    // Run all benchmarks
    bench_market_order_scaling(&mut criterion);
    bench_limit_order_insertion(&mut criterion);
    bench_mixed_book_operations(&mut criterion);
    bench_price_level_impact(&mut criterion);
    bench_order_cancellation(&mut criterion);
    bench_short_covering_scenarios(&mut criterion);
    bench_negative_inventory_patterns(&mut criterion);

    // Export all CSV results at the end
    export_all_results();
}

criterion_group!(
    benches,
    bench_market_order_scaling,
    bench_limit_order_insertion,
    bench_mixed_book_operations,
    bench_price_level_impact,
    bench_order_cancellation,
    bench_short_covering_scenarios,
    bench_negative_inventory_patterns
);

//criterion_main!(benches);
fn main() {
    // build a Criterion instance
    let mut c = Criterion::default();
    // run each suite
    bench_market_order_scaling(&mut c);
    bench_limit_order_insertion(&mut c);
    bench_mixed_book_operations(&mut c);
    bench_price_level_impact(&mut c);
    bench_order_cancellation(&mut c);
    bench_short_covering_scenarios(&mut c);
    bench_negative_inventory_patterns(&mut c);
    // finally export
    export_all_results();
}
