use criterion::{Bencher, Criterion, criterion_group, criterion_main};
use crossbeam_channel::unbounded;
use dashmap::DashMap;
use market_simulator::Agent;
use market_simulator::DumbAgent;
use market_simulator::simulation::orchestra::MarketState;
use market_simulator::simulation::orchestra::{ConcurrentMarketState, ShadowBookHandle};
use market_simulator::simulators::order_book::OrderBook;
use market_simulator::stocks::StockMarket;
use market_simulator::types::order::{Order, Side};
//use std::collections::HashMap;
use std::sync::{Arc, RwLock};

// --- 1. Matching Engine & Order Book Performance ---

fn order_book_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("Order Book Throughput");

    group.bench_function("mixed_orders", |b: &mut Bencher| {
        let mut order_book = OrderBook::new();
        let orders: Vec<Order> = (0..1000)
            .map(|i| {
                let side = if i % 2 == 0 { Side::Buy } else { Side::Sell };
                let price = if side == Side::Buy { 100 } else { 101 };
                Order {
                    id: i,
                    agent_id: (i % 10) as usize,
                    stock_id: 1,
                    side,
                    price,
                    volume: 10,
                    filled: 0,
                }
            })
            .collect();

        b.iter(|| {
            for order in &orders {
                let mut o = order.clone();
                order_book.process_limit_order(&mut o);
            }
        })
    });
}

fn order_latency(c: &mut Criterion) {
    let mut group = c.benchmark_group("Order Latency");

    group.bench_function("add_limit_order", |b: &mut Bencher| {
        let mut order_book = OrderBook::new();
        let mut order = Order {
            id: 0,
            agent_id: 0,
            stock_id: 1,
            side: Side::Buy,
            price: 100,
            volume: 10,
            filled: 0,
        };
        b.iter(|| {
            order_book.process_limit_order(&mut order);
        })
    });

    group.bench_function("add_market_order", |b: &mut Bencher| {
        let mut order_book = OrderBook::new();
        let mut limit_order = Order {
            id: 0,
            agent_id: 0,
            stock_id: 1,
            side: Side::Sell,
            price: 100,
            volume: 10,
            filled: 0,
        };
        order_book.process_limit_order(&mut limit_order);

        b.iter(|| {
            order_book.process_market_order(0, 1, Side::Buy, 10); // Add taker_order_id: 0
        })
    });
}

// --- 2. Agent and Simulation Logic ---

fn agent_decision_making(c: &mut Criterion) {
    let mut group = c.benchmark_group("Agent Decision Making");

    group.bench_function("dumb_agent_update", |b: &mut Bencher| {
        let (tx_order, _rx_order) = unbounded();
        let (_tx_ack, rx_ack) = unbounded();
        let (_tx_trade, rx_trade) = unbounded();
        let stock_market = StockMarket::new();
        let concurrent_state = ConcurrentMarketState {
            order_books: DashMap::new(),
            stocks: stock_market,
            last_traded_price: DashMap::new(),
            cumulative_volume: DashMap::new(),
        };
        let market_state = MarketState::from_concurrent(&concurrent_state);
        let view_handle: ShadowBookHandle = Arc::new(RwLock::new(market_state));
        let mut agent = DumbAgent::new(0, tx_order, rx_ack, rx_trade, view_handle);

        b.iter(|| {
            agent.decide_actions();
        })
    });
}

// --- 3. Scalability Benchmarks ---

fn scalability_with_agents(c: &mut Criterion) {
    let mut group = c.benchmark_group("Scalability with Agents");

    for num_agents in [10, 100, 500].iter() {
        group.bench_with_input(
            criterion::BenchmarkId::from_parameter(num_agents),
            num_agents,
            |b, &num_agents| {
                b.iter(|| {
                    let (tx_order, _rx_order) = unbounded();
                    let stock_market = StockMarket::new();
                    let concurrent_state = ConcurrentMarketState {
                        order_books: DashMap::new(),
                        stocks: stock_market,
                        last_traded_price: DashMap::new(),
                        cumulative_volume: DashMap::new(),
                    };
                    let market_state = MarketState::from_concurrent(&concurrent_state);
                    let view_handle: ShadowBookHandle = Arc::new(RwLock::new(market_state));
                    let mut agents: Vec<DumbAgent> = Vec::new();
                    for i in 0..num_agents {
                        let (_tx_ack, rx_ack) = unbounded();
                        let (_tx_trade, rx_trade) = unbounded();
                        agents.push(DumbAgent::new(
                            i as usize,
                            tx_order.clone(),
                            rx_ack,
                            rx_trade,
                            view_handle.clone(),
                        ));
                    }
                });
            },
        );
    }
}

criterion_group!(
    benches,
    order_book_throughput,
    order_latency,
    agent_decision_making,
    scalability_with_agents
);
criterion_main!(benches);
