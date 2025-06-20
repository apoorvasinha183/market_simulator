# Market Simulator

A high-performance financial market simulator written in Rust for designing, training, and evaluating trading agents—classical or reinforcement-learning based—under realistic limit-order-book conditions.

## Overview

**Market Simulator** is a comprehensive financial market simulation engine designed for researchers, quantitative developers, and algorithmic traders. The system provides exchange-grade order matching mechanics with microsecond-level performance characteristics, enabling realistic backtesting and agent development scenarios.

### Key Features

* **Performance and Safety** – Implemented in pure safe Rust with core hot-loop execution in the low-microsecond range on commodity hardware
* **Exchange-Grade Mechanics** – Price-time FIFO matching, partial fills, order cancellations, and multi-level order sweeps
* **Pluggable Architecture** – Engine packaged as reusable crate with minimal `Agent` trait for runtime injection
* **Synthetic Market Data** – Built-in generators for realistic price paths using Geometric Brownian Motion and Black-Scholes option pricing
* **Multi-Asset Support** – Per-symbol sharded order books with cross-asset interaction capabilities
* **Market Sentiment Integration** – Advanced sentiment modeling affecting market participant behavior
* **Comprehensive Testing** – Unit tests for edge cases, Criterion benchmarks, and visualization tools

## Architecture

### Core Components

The simulator is structured around several key modules:

**Order Book Engine** (`src/simulators/order_book.rs`)
- High-performance limit order book implementation
- Price-time priority matching
- Support for market orders, limit orders, and cancellations
- Multi-level order sweeping capabilities

**Market Simulation** (`src/market.rs`)
- Central market state management
- Tick-by-tick simulation orchestration
- Agent coordination and execution scheduling

**Agent Framework** (`src/agents/`)
- Pluggable agent architecture
- Reference implementations for market makers, momentum traders, and random flow
- Standardized interface for custom agent development

**Pricing Models** (`src/pricing/`)
- Black-Scholes option pricing implementation
- Greeks calculation for risk management
- Fair value estimation utilities

**Market Data Generation** (`src/simulators/gbm.rs`)
- Geometric Brownian Motion price path generation
- Configurable volatility and drift parameters
- Realistic intraday price movement simulation

**Market Sentiment** (`src/sentiment.rs`)
- Sentiment factor modeling
- Impact on agent behavior and market dynamics
- Configurable sentiment scenarios

### Data Types

The system uses strongly-typed representations for all financial instruments and market operations:

**Stock Management** (`src/stocks/`)
- Stock symbol definitions and metadata
- Multi-asset portfolio management
- Cross-asset correlation modeling

**Type System** (`src/types/` and `src/shared_types.rs`)
- Comprehensive type definitions for orders, trades, and market events
- Memory-efficient representations optimized for high-frequency operations

## Performance Characteristics

### Benchmark Results

*Configuration: 50,000 resting orders, 25,000-share market sweep*
*Hardware: Apple M1 Pro, single core, Rust 1.87 release profile*
```

Execution Time: [38.8 µs 40.1 µs 41.3 µs] (Criterion, 50 samples)
Throughput: 1.26 G order-operations·s⁻¹
```
*High-volume scenario: 1,000,000 resting orders, 100,000 share sweep*
```

Execution Time: [2.72 ms 2.85 ms 2.98 ms]
```
Performance figures include complete order book construction; pure matching latency is significantly lower.

## Project Development Status

| Phase | Description | Status |
|-------|-------------|---------|
| **0 – Foundation** | Repository scaffolding, CI pipeline, basic crate layout | **Complete** |
| **1 – Single-Asset Price Path** | Geometric Brownian Motion time series generation | **Complete** |
| **2 – Fair Option Pricing** | Black-Scholes model implementation (pricing and Greeks) | **Complete** |
| **3 – Order Book Dynamics** | Iterative price-level generation and matching engine | **Complete** |
| **4 – Market Participants** | Ensemble of agents (market makers, institutional flow, retail) | **Complete** |
| **5 – RL Integration** | Streaming interface (ZeroMQ/gRPC) for external policy control | *Deferred* |
| **6 – Multi-Asset Support** | Per-symbol sharded books with cross-asset interaction | **Complete** |
| **7 – Options Market** | Dedicated options pricing and trading infrastructure | *Planned* |

## Installation and Setup

### Prerequisites

- Rust 1.87.0 or later
- Cargo package manager

### Building the Project
```
bash
# Clone the repository
git clone <repository-url>
cd market-simulator

# Build in release mode for optimal performance
cargo build --release

# Run comprehensive test suite
cargo test

# Generate documentation
cargo doc --no-deps --open
```
### Running Benchmarks

Execute performance benchmarks with Criterion:
```
bash
# Full benchmark suite
cargo bench

# Quick benchmark for CI environments
cargo bench --bench order_book -- --sample-size 30 --measurement-time 6

# Enhanced order book benchmarks
cargo bench --bench order_book_enhanced
```

## Visualization Tools

The project includes several visualization utilities:

**Order Book Visualizer** (`src/bin/visual_order.rs`)
- Real-time order book depth visualization
- Price level analysis
- Order flow monitoring

**Market Data Visualizer** (`src/bin/visualizer.rs`)
- GBM price path visualization
- Option pricing curves
- Historical data analysis

Run visualizers with:

```bash
cargo run --bin visual_order
cargo run --bin visualizer
```



## Directory Structure

```
src/
├── agents/              # Reference agent implementations
├── bin/                 # Executable utilities and visualizers
├── pricing/             # Financial pricing models (Black-Scholes)
├── simulators/          # Core simulation engines
│   ├── order_book.rs    # Limit order book matching engine
│   └── gbm.rs           # Geometric Brownian Motion generator
├── stocks/              # Stock symbol and metadata management
├── types/               # Type definitions and data structures
├── lib.rs               # Library entry point
├── market.rs            # Central market simulation orchestrator
├── sentiment.rs         # Market sentiment modeling
└── shared_types.rs      # Common type definitions

benches/                 # Performance benchmarks
├── order_book.rs        # Basic order book benchmarks
└── order_book_enhanced.rs # Advanced benchmarking scenarios

benchmark_pdfs/          # Generated benchmark reports
criterion_pdf.py         # Benchmark visualization utilities
```


## Configuration

The simulator supports extensive configuration through environment variables and configuration files. Key parameters include:

- Market microstructure settings (tick size, lot size)
- Agent population and behavior parameters
- Simulation duration and frequency
- Performance optimization flags


## Dependencies

The project leverages several high-quality Rust crates:

- **eframe/egui**: Modern GUI framework for visualization tools
- **criterion**: Statistical benchmarking framework
- **serde**: Serialization framework for data persistence
- **rand**: High-performance random number generation
- **statrs**: Statistical functions and distributions
- **parking_lot**: High-performance synchronization primitives

## License

[License information to be specified]

## Documentation

Comprehensive API documentation is available via `cargo doc --no-deps`. Generated documentation includes:

- Complete API reference
- Usage examples
- Performance characteristics
- Architecture diagrams

Access documentation locally at `target/doc/market_simulator/index.html` or through the project's documentation hosting service.
