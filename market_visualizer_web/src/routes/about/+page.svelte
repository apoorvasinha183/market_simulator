<script lang="ts">
  import { onMount } from 'svelte';

  let markdownContent = `# Market Simulator: A Deep Dive into its Architecture

The \`market_simulator\` project is a high-performance, modular, and concurrently designed market simulation engine built primarily in Rust. Its architecture is meticulously crafted to handle complex market dynamics, facilitate diverse agent behaviors, and provide real-time interaction and visualization capabilities. This document provides an in-depth exploration of its core components, their interactions, and the underlying design principles.

## 1. Core Simulation Engine: The \`market_simulator\` Library

The heart of the system resides within the \`src/\` directory, forming the \`market_simulator\` Rust library. This library encapsulates all fundamental simulation logic, financial models, and market mechanics. It leverages Rust's strong type system, ownership model, and concurrency primitives to ensure both performance and correctness.

### 1.1. Agents (\`src/agents/\`)

Agents are the active participants in the simulated market, each embodying a specific trading strategy. Their design is highly polymorphic, adhering to the \`Agent\` trait.

*   **\`agent_trait.rs\`**: This file defines the \`pub trait Agent: Send + Sync\`.
    *   \`Agent\`: The fundamental contract for all market participants. It specifies methods like \`decide_actions\` (the agent's core decision-making loop), \`buy_stock\`/\`sell_stock\` (high-level market order execution), and various methods for order and portfolio management (\`acknowledge_order\`, \`update_portfolio\`, \`evaluate_port\`, \`get_pending_orders\`, \`cancel_open_order\`).
    *   \`Send + Sync\`: These are crucial Rust marker traits. \`Send\` ensures an agent instance can be safely moved to another thread (as agents typically run in their own threads). \`Sync\` ensures an agent can be safely accessed via shared references from multiple threads concurrently (e.g., when its state is observed by the \`Orchestra\` or other components).
    *   \`run(&mut self)\`: This method defines the agent's execution lifecycle, typically involving spawning background threads for continuous processing (like listening for acknowledgments and trades) and then entering a loop to repeatedly call \`decide_actions\`.

*   **\`market_maker_agent.rs\`**: Implements a sophisticated \`MarketMakerAgent\` that provides liquidity.
    *   **Internal State**: Utilizes \`Arc<RwLock<...>>\` extensively for thread-safe shared state (e.g., \`inventory\`, \`open_orders\`, \`cash\`, \`last_quoted_prices\`). \`RwLock\` is chosen to allow multiple concurrent readers and exclusive writers, optimizing for read-heavy access patterns.
    *   **Communication**: Communicates with the \`Market\` via \`crossbeam_channel\`s (\`order_channel\` for sending requests, \`ack_channel\` for acknowledgments, \`port_channel\` for trades). It receives a \`ShadowBookHandle\` (\`Arc<RwLock<MarketState>>\`) for a read-only view of the market.
    *   **Concurrency**: Spawns dedicated threads for \`run_portfolio_updater_internal\` (processing trades) and \`run_ack_listener_internal\` (processing order acknowledgments).
    *   **\`decide_actions_internal\`**: This is the core market-making logic. It's highly concurrent:
        *   It uses a \`ticks_until_active\` counter for a warm-up period.
        *   It iterates through all stocks and *spawns a new thread for each stock* to calculate quoting decisions concurrently. This is a significant performance optimization for multi-stock simulations.
        *   **Bootstrapping**: Places initial "seed" orders across multiple price levels to establish initial liquidity.
        *   **Continuous Quoting**:
            *   Handles "unsticking" the market by placing orders if one side of the book is empty.
            *   Calculates new bid/ask prices based on \`last_traded_price\` or \`initial_price\`.
            *   **Inventory Skew**: Adjusts quotes based on its current \`inventory\` (\`MM_SKEW_FACTOR\`). If long, it skews quotes lower to encourage selling; if short, it skews higher to encourage buying.
            *   **Requote Threshold**: Avoids unnecessary order cancellations/placements if price changes are below \`MM_REQUOTE_THRESHOLD_BPS\`.
            *   **Cancel-and-Replace**: If requoting is needed, it first cancels all its existing open orders for that stock, then places new limit orders. This is a standard market-making practice.
    *   **\`run\` method**: Orchestrates the background threads and enters a \`loop\` that repeatedly calls \`decide_actions\`, with a small \`thread::sleep\` to yield control.

*   **\`momentum_agent.rs\`**: Implements a \`MomentumAgent\` that trades on price trends.
    *   **Internal State**: Key difference is \`price_history: Arc<RwLock<HashMap<u64, VecDeque<f64>>>>\`, which stores a rolling window of past prices for each stock.
    *   **\`decide_actions_internal\`**:
        *   **Probabilistic Action**: \`rng.gen_bool(MOMENTUM_AGENT_ACTION_PROB)\` introduces randomness in whether the agent acts in a given tick.
        *   **Momentum Calculation**: Calculates percentage price change over \`MOMENTUM_AGENT_MOMENTUM_WINDOW\` using \`VecDeque\`'s \`front()\` and \`back()\`.
        *   **Decision**: If \`price_change_pct\` exceeds \`MOMENTUM_AGENT_MOMENTUM_THRESHOLD\` (positive or negative), it decides to \`Buy\` or \`Sell\`.
        *   **Order Placement**: Places \`LimitOrder\`s with a random \`offset\` (\`MOMENTUM_AGENT_PRICE_OFFSET_MIN/MAX\`) and \`volume\` (\`MOMENTUM_AGENT_VOL_MIN/MAX\`).
        *   **Cash Check**: Performs a basic cash check before placing buy orders.
    *   **\`run\` method**: Similar to \`MarketMakerAgent\`, but with a longer \`thread::sleep\` (100 nanoseconds vs. 10 nanoseconds), reflecting the slower nature of momentum trading.

### 1.2. Market Engine (\`src/market.rs\`)

The \`Market\` struct is the central matching engine and orchestrator, designed as a "dumb" manager of order books. It routes orders, aggregates trades, and disseminates market information.

*   **Internal State**:
    *   \`order_txs: HashMap<u64, Sender<OrderRequest>>\`: Maps \`stock_id\` to a \`Sender\` for that stock's \`AsyncOrderBook\`. This is the core routing mechanism.
    *   \`order_rx: Receiver<OrderRequest>\`: The single input channel for all \`OrderRequest\`s from all agents.
    *   \`order_id_counter: Arc<RwLock<u64>>\`: Generates unique order IDs.
    *   \`order_id_to_stock_id_map: Arc<RwLock<HashMap<u64, u64>>>\`: Maps \`order_id\` to \`stock_id\`, crucial for routing cancellations and trade processing.
    *   \`agent_channels: Arc<HashMap<usize, AgentResponseChannels>>\`: Stores agent-specific channels for sending acknowledgments and trades.
    *   \`shadow_update_txs\` / \`vip_shadow_update_txs\`: Channels for sending \`ShadowEvent\`s to external market view consumers.
    *   \`last_traded_price: Arc<RwLock<HashMap<u64, f64>>>\`: Global, thread-safe map of last traded prices.

*   **\`new\` function**:
    *   Initializes \`last_traded_price\` with initial stock prices.
    *   **Per-Stock \`AsyncOrderBook\` Spawning**: For each stock, it creates an \`AsyncOrderBook\` (which spawns its own thread) and stores its \`Sender<OrderRequest>\` in \`order_txs\`.
    *   **Trade Aggregation**: Spawns a thread for *each* \`AsyncOrderBook\`'s trade output (\`stock_trade_rx\`) to forward all trades to a single, central \`trade_tx\` channel. This aggregates all trades into one stream.
    *   **\`spawn_trade_processor\`**: Launches a dedicated thread to process this aggregated trade stream.

*   **\`spawn_trade_processor\`**: This background thread is critical.
    *   Continuously receives \`Trade\` messages from the aggregated stream.
    *   Updates the global \`last_traded_price\`.
    *   Removes the \`maker_order_id\` from \`order_id_to_stock_id_map\` (as it's now traded).
    *   Broadcasts \`MarketEvent::TradeOccurred\` to the central event bus.
    *   Sends the \`Trade\` to the specific \`trade_tx\` channels of both the \`taker_agent_id\` and \`maker_agent_id\`.
    *   Forwards the \`Trade\` to the \`trade_to_candle_tx\` for candlestick data generation.

*   **\`process_request(mut req: OrderRequest)\`**: The core request handler.
    *   Generates a unique \`order_id\` and assigns it to the \`req\`.
    *   **Pattern Matching**: Handles \`LimitOrder\`, \`MarketOrder\`, and \`CancelOrder\` requests.
        *   For \`LimitOrder\` and \`MarketOrder\`, it sends an \`Order\` acknowledgment back to the agent via \`ack_tx\` and records the \`order_id\` to \`stock_id\` mapping.
        *   For \`MarketOrder\`, it determines a price based on \`last_traded_price\` (a simplification).
        *   For \`CancelOrder\`, it uses \`order_id_to_stock_id_map\` to find the \`stock_id\` and remove the order.
    *   **Dispatch**: Routes the \`OrderRequest\` to the correct \`AsyncOrderBook\` via \`order_txs\`.
    *   **Shadow Event Dispatch**: Sends \`ShadowEvent\`s to \`shadow_update_txs\` for external views.

*   **\`Marketable\` trait implementation**: The \`Market\` implements \`run\` which continuously receives and processes \`OrderRequest\`s, making it an event-driven, reactive component. Other \`Marketable\` methods are stubbed, reinforcing that \`Market\` is a matching engine, not a simulation model.

### 1.3. Simulators (\`src/simulators/\`)

This module contains the actual order book implementations and other simulation models.

*   **\`order_book.rs\`**: Defines the synchronous \`OrderBook\` struct. This is the foundational, single-threaded matching logic.
    *   **Data Structures**: Uses \`BTreeMap<u64, PriceLevel>\` for \`bids\` and \`asks\` (sorted by price) and \`VecDeque<Order>\` within \`PriceLevel\` (for time priority). \`HashMap<u64, (Side, u64)>\` for \`order_id_map\` for fast lookups.
    *   **\`process_market_order\`**: Implements aggressive matching against the best available prices, consuming liquidity.
    *   **\`process_limit_order\`**: Attempts to match against existing orders. If not fully filled, the remaining volume is added to the book (\`add_limit_order\`).
    *   **\`cancel_order\`**: Removes an order, including an \`agent_id\` check for ownership.
    *   This \`OrderBook\` is the core algorithm that \`AsyncOrderBook\` wraps for concurrency.

*   **\`async_order_book.rs\`**: Defines the \`AsyncOrderBook\` struct, which wraps the \`OrderBook\` logic for concurrent execution.
    *   **Concurrency Model**: Each \`AsyncOrderBook\` instance runs in its *own dedicated thread*. This is a key design choice: it avoids complex internal locking within the matching logic itself, as all operations on \`bids\`, \`asks\`, and \`orders\` are sequential within that thread.
    *   **Communication**: Uses \`crossbeam_channel\`s for input (\`order_rx: Receiver<OrderRequest>\`) and output (\`trade_tx: Sender<Trade>\`).
    *   **\`new\` function**: Spawns the dedicated thread and returns the \`Sender\` and \`Receiver\` channels.
    *   **\`process_request\`**: Receives \`OrderRequest\`s and delegates to the internal \`OrderBook\`'s matching methods.
    *   This design allows the \`Market\` to scale horizontally by simply creating more \`AsyncOrderBook\` threads, each managing a different stock independently.

### 1.4. Pricing (\`src/pricing/\`)

*   **\`black_scholes.rs\`**: Implements the Black-Scholes model for European option pricing, including calculations for "Greeks" (Delta, Gamma, Theta, Vega, Rho). This provides financial modeling capabilities within the simulation.

### 1.5. Sentiment Engine (\`src/sentiment_engine/\`)

*   **\`sentiment_collector.rs\`**: This module is designed to integrate market sentiment. It would be responsible for generating or collecting sentiment data, which can then influence agent behavior or market dynamics.

### 1.6. Data Types (\`src/types/\`, \`src/shared_types/\`, \`src/stocks/\`)

These modules define the fundamental data structures that ensure type safety and consistency across the simulation.
*   \`order.rs\`: Defines \`Order\`, \`OrderRequest\`, \`Side\`, and \`Trade\`.
*   \`candle.rs\`: Represents candlestick data.
*   \`definitions.rs\`: Defines \`Stock\`, \`Symbol\`, and \`StockMarket\`.
*   \`shared_types.rs\`: Contains common types like \`OptionType\`.

### 1.7. Events (\`src/events.rs\`)

*   **\`MarketEvent\` enum**: Defines the central event bus vocabulary.
    *   \`TradeOccurred(Trade)\`: Emitted when a trade is executed.
    *   \`SentimentUpdate { stock_id: u64, score: f64 }\`: Signals changes in stock sentiment.
    *   \`Heartbeat\`: A periodic signal for time progression and synchronization.
*   This event-driven approach decouples components, enhances scalability, and improves observability.

### 1.8. Simulation Orchestration (\`src/simulation/orchestra.rs\`)

The \`Orchestra\` is the central coordinator, setting up and launching all simulation components.

*   **\`Shadow Book Infrastructure\`**: Provides a consistent, thread-safe view of the market state to agents and external consumers.
    *   **\`ShadowEvent\`**: Events sent from \`Market\` to \`ShadowCoordinator\` to update the view.
    *   **\`ConcurrentMarketState\`**: The mutable "back-buffer" using \`DashMap\` for concurrent updates by builder threads.
    *   **\`MarketState\`**: The immutable "front-buffer" (\`Arc<RwLock<MarketState>>\`) that agents read. It's a snapshot created from \`ConcurrentMarketState\`.
    *   **\`ShadowCoordinator\`**: Manages the double-buffering. It spawns \`run_builder_thread\`s (one per stock) to process \`ShadowEvent\`s and update the \`ConcurrentMarketState\`. A separate thread periodically swaps the \`ConcurrentMarketState\` into the \`MarketState\` (front-buffer) by acquiring a \`RwLock\` write lock, ensuring agents always read a consistent snapshot.

*   **\`new\` function**:
    *   Initializes \`StockMarket\`, central \`order_tx\`/\`rx\` and \`event_tx\`/\`rx\` channels.
    *   Sets up \`normal_shadow_senders\`/\`receivers\` and \`premium_shadow_senders\`/\`receivers\` for \`ShadowCoordinator\`s (allowing different update frequencies/details).
    *   Launches two \`ShadowCoordinator\` instances (normal and premium).
    *   Instantiates all agents based on \`AgentType\`, assigning them their communication channels and a \`ShadowBookHandle\` (either normal or premium view).
    *   Instantiates the central \`Market\`, connecting it to all agent channels and shadow book senders.

*   **\`run\` function**: Launches all major components into their own threads:
    *   A \`Heartbeat\` sender thread.
    *   A \`SentimentEngine\` thread.
    *   A \`CandleAnalyzer\` thread (processes trades into candlestick data).
    *   The \`Market\`'s main loop thread.
    *   Individual threads for each \`Agent\`'s \`run\` method.
*   This ensures high parallelism and responsiveness across the entire simulation.

## 2. External Communication: gRPC Server

The project exposes its functionality via a gRPC server, enabling real-time interaction with external applications.

### 2.1. gRPC Server (\`src/bin/grpc_server.rs\`)

This binary launches the simulation and delegates gRPC server hosting to the \`CustomerAgent\`.

*   It defines the list of \`AgentType\`s to participate, including \`CustomerAgent\`.
*   It creates and launches the \`Orchestra\` in a separate thread.
*   The main thread then simply waits for \`stdin\` to close for graceful shutdown.

### 2.2. Customer Agent as gRPC Host (\`src/agents/customer_agent.rs\`)

The \`CustomerAgent\` is unique as it hosts the gRPC \`MarketGateway\` service.

*   **Internal State**: Includes \`grpc_tx\` (for receiving incoming gRPC requests from the server implementation) and \`grpc_response_txs\` (a map of client-specific channels for sending responses back to gRPC clients).
*   **\`run\` method**:
    *   Spawns background threads for \`run_grpc_listener_internal\` (processes incoming \`FromPython\` requests, translates them to \`OrderRequest\`s, and sends to \`Market\`) and \`run_market_data_broadcaster_internal\` (periodically reads \`MarketState\` and sends \`MarketUpdate\`s to all connected gRPC clients).
    *   **gRPC Server Startup**: Crucially, it creates a \`tokio\` multi-threaded runtime and uses \`tonic::transport::Server::builder().serve(...)\` to start the gRPC server on \`[::0]:50051\`. This means the gRPC server runs within the \`CustomerAgent\`'s thread context, managed by \`tokio\`.
*   This design allows external Python clients to submit orders and receive real-time market data and trade updates via a bidirectional streaming RPC (\`EventStream\`).

### 2.3. Python Client (\`python_client/\`)

This directory contains the client-side components for interacting with the Rust gRPC server.
*   **Generated Client Code**: Automatically generated Python code from \`proto/market_gateway.proto\`.
*   **Client Logic**: \`broker.py\`, \`rl_gateway.py\` provide Python-specific logic for order submission and data reception.
*   **Operational Scripts**: \`run_broker.py\`, \`run_multi_agent_demo.py\`, \`run_sanity_check.py\` demonstrate client interactions.

## 3. Visualization Layer

The project offers both web-based and desktop-based visualization tools.

### 3.1. Web Visualizer (\`market_visualizer_web/\`)

A SvelteKit application providing a graphical interface for real-time market data.
*   Connects to the Rust backend (likely via WebSockets, as seen in \`+page.svelte\`).
*   Displays order book data, trade history, candlestick charts, and key market metrics.

### 3.2. Desktop Visualizers (\`src/bin/visualizer.rs\`, \`src/bin/visual_order.rs\`)

Rust executables using \`eframe\` and \`egui_plot\` for desktop-based graphical analysis. These provide high-performance, potentially more detailed, or debugging-oriented visualizations.

## 4. Performance & Quality Assurance

*   **Benchmarking (\`benches/\`)**: Uses \`criterion\` for performance benchmarks (e.g., \`order_book.rs\`, \`comprehensive_benchmark.rs\`). \`criterion_pdf.py\` suggests automated report generation.
*   **Testing (\`tests/\`)**: Comprehensive unit and integration tests (\`advanced_simulation_test.rs\`, \`customer_agent_integration_test.rs\`, \`simulation_test.rs\`) ensure correctness and reliability.

## 5. Build & Deployment

*   **\`Cargo.toml\`**: Rust project manifest.
*   **\`Dockerfile\` / \`docker-compose.yml\`**: Containerization for deployment.
*   **\`build.rs\`**: Rust build script, likely for compiling Protocol Buffers definitions.

## Architectural Principles: A Summary

*   **Modularity**: Clear separation of concerns (agents, market, simulators, pricing, events, visualization) promotes maintainability and extensibility.
*   **Performance**: Achieved through Rust's efficiency, extensive use of concurrency (\`tokio\`, \`crossbeam_channel\`, \`DashMap\`, \`RwLock\`), and dedicated threads for critical paths (per-stock order books, trade processing).
*   **Extensibility**: The \`Agent\` trait, gRPC API, and event-driven design allow for easy integration of new strategies, data sources, and external systems.
*   **Observability**: Comprehensive visualization tools, event broadcasting, and benchmarking provide deep insights into simulation behavior.
*   **Concurrency**: Pervasive use of asynchronous programming and concurrent data structures enables the simulation to handle high loads and complex interactions.
*   **Consistency**: The double-buffered \`MarketState\` ensures agents and external views always receive a consistent snapshot of the rapidly changing market.

This architecture provides a robust, scalable, and analyzable platform for market simulation and research.`;

  let renderedHtml = '';

  onMount(async () => {
    const { marked } = await import('marked');
    renderedHtml = marked(markdownContent);
  });
</script>

<div class="container">
  <div class="content">
    {@html renderedHtml}
  </div>
</div>

<style>
  .container {
    max-width: 900px;
    margin: 0 auto;
    padding: 20px;
    font-family: 'Arial', sans-serif;
    line-height: 1.6;
    color: #333;
  }

  .content :global(h1),
  .content :global(h2),
  .content :global(h3),
  .content :global(h4),
  .content :global(h5),
  .content :global(h6) {
    color: #2c3e50;
    margin-top: 1.5em;
    margin-bottom: 0.5em;
  }

  .content :global(h1) {
    font-size: 2.5em;
    border-bottom: 2px solid #eee;
    padding-bottom: 10px;
    margin-bottom: 20px;
  }

  .content :global(h2) {
    font-size: 2em;
    border-bottom: 1px solid #eee;
    padding-bottom: 5px;
    margin-bottom: 15px;
  }

  .content :global(h3) {
    font-size: 1.5em;
  }

  .content :global(ul) {
    list-style-type: disc;
    margin-left: 20px;
  }

  .content :global(code) {
    background-color: #f4f4f4;
    padding: 2px 4px;
    border-radius: 4px;
    font-family: 'Courier New', monospace;
    color: #c7254e;
  }

  .content :global(pre) {
    background-color: #f4f4f4;
    padding: 10px;
    border-radius: 5px;
    overflow-x: auto;
  }

  .content :global(pre code) {
    background-color: transparent;
    padding: 0;
    color: #333;
  }

  .content :global(p) {
    margin-bottom: 1em;
  }
</style>