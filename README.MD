# Market Simulation Platform

Multi-threaded financial market simulator implementing realistic market microstructure. Models concurrent agent interactions, information asymmetry, and emergent market behaviors through limit order book dynamics.

Core architecture supports ETF arbitrage, sentiment-driven trading, thermodynamic market models, and reinforcement learning integration.

## Architecture

![Simulation Demo](simulation_demo.gif)

### Core Components

**Multi-threaded Actor System**
- Actor-based architecture with thread-per-component isolation
- Channel-based inter-component communication
- Asynchronous order matching engine with per-stock order books

**Shadow Book System**
- High-frequency read-only market state snapshots
- Tiered data access (normal/premium) for information asymmetry modeling
- Lock-free concurrent data structures for performance

**Agent Framework**
- 13 distinct agent types with specialized behaviors
- Thread-isolated decision making and portfolio management
- Configurable agent populations and parameters

**Market Structure**
- 25 stocks with realistic ownership distributions
- 5 ETFs with constituent-based NAV calculation
- Sentiment-driven price discovery via UDP multicast

### Agent Types

**Market Infrastructure**
- `MarketMaker`: Provides liquidity with bid-ask spreads
- `ETFMaintenanceAgent`: Authorized participant arbitrage (10bps threshold)
- `ETFAgent`: General ETF arbitrage (50bps threshold)

**Institutional Participants**
- `WhaleAgent`: Large block trading with market impact
- `Thermodynamic`: Physics-based trading using temperature/entropy models
- `MomentumAgent`: Trend-following strategies

**Behavioral Models**
- `PanicAgent`: Feedback loop generation during market stress
- `Astrologer`: Sentiment-based decision making
- `CustomerAgent`: External gRPC interface host

**Utility Agents**
- `DumbMarket`/`DumbLimit`: Basic order placement for testing
- `IPO`: Initial public offering simulation
- `WebProxyAgent`: Web interface integration

### External Interfaces

**gRPC Server**
- Bidirectional streaming for external client integration
- Real-time order placement and market data access
- Python client library with reinforcement learning support

**Web Interface**
- Real-time order book visualization
- Candlestick charts with multiple timeframes
- Portfolio tracking and trade history

**Data Integration**
- UDP multicast sentiment injection
- CSV-based stock universe configuration
- YAML-based simulation parameters

## Order Execution Flow

1. **Decision**: Agent reads shadow book snapshot, executes trading logic
2. **Submission**: `OrderRequest` sent via channel to central `Market`
3. **Routing**: Market assigns ID, sends acknowledgment, routes to stock-specific order book
4. **Matching**: Asynchronous order book processes against canonical book state
5. **Settlement**: Trade confirmations sent to involved agents
6. **State Update**: Agent portfolio threads update cash/inventory positions
7. **Broadcast**: Shadow coordinator updates market state snapshots

## Performance Characteristics

**Throughput**: 100,000+ trades processed with sub-millisecond latency
**Concurrency**: 25+ concurrent agents with independent decision cycles
**Memory**: Lock-free data structures minimize contention overhead
**Scalability**: Linear scaling with additional stocks/agents

## Technology Stack

**Core Runtime**
- Rust with `tokio` async runtime
- `crossbeam-channel` for actor communication
- `dashmap`/`parking_lot` for concurrent data structures

**Networking**
- `tonic`/`prost` for gRPC services
- `axum` for web server
- UDP multicast for sentiment data

**Visualization**
- `egui`/`eframe` for native GUI
- Svelte/TypeScript for web interface
- Real-time WebSocket data streaming

**External Integration**
- Python gRPC client with RL framework
- Docker containerization
- `criterion` benchmarking suite

## Usage

### Prerequisites
- Rust 1.70+
- Python 3.10+
- Node.js 18+ (for web interface)

### Execution Modes

**Native GUI Visualizer**
```bash
cargo run --release --bin visual_order
```

**gRPC Server Mode**
```bash
cargo run --release --bin grpc_server
```

**ETF Crisis Simulation**
```bash
cargo run --release --bin visual_etf_crisis
```

**Market Manipulation Testing**
```bash
cargo run --release --bin test_market_manipulation
```

### Python Client Integration
```bash
cd python_client
pip install -r requirements.txt
python run_broker.py
```

### Web Interface
```bash
cd market_visualizer_web
npm install && npm run dev
```

### Docker Deployment
```bash
docker-compose up --build
```

## Project Structure

*   `./`: Root directory, containing `Cargo.toml`, `docker-compose.yml`, etc.
*   `src/`: The core Rust simulation backend.
    *   `agents/`: Implementations of agent behaviors.
    *   `bin/`: Entry points for executable binaries (`grpc_server`, visualizers).
    *   `market/`: The core market routing logic.
    *   `simulation/`: The `Orchestra` and the "Shadow Book" architecture.
    *   `simulators/`: The `OrderBook` implementation.
*   `proto/`: The gRPC service contract (`market_gateway.proto`).
*   `python_client/`: The Python client for interacting with the simulation.
*   `market_visualizer_web/`: The Svelte-based web frontend.
*   `sentiment_service/`: A separate Rust project that can broadcast sentiment data.
*   `benches/`: Performance benchmarks for the Rust core.
*   `tests/`: Integration tests for the Rust core.

## License

This project is licensed under the MIT License. See the `LICENSE` file for details.


---

### A Note on Tooling
The architecture and core system design of this project are my own. To accelerate development and focus on the higher-level design, AI assistants were used for generating boilerplate code, documentation, and unit tests.