# 🚀 Market Simulator

A high-performance, multi-threaded financial market simulator written in Rust. It's designed for designing, training, and evaluating trading agents—whether they're based on classical algorithms or modern reinforcement learning—in a realistic, high-fidelity limit order book environment.

## What is this?

Have you ever wanted to see how a trading idea would *really* perform? This project lets you do just that. It's a complete, end-to-end simulation of a stock market, from the exchange that matches orders to the different types of traders that populate it. You can write your own automated trading agents, drop them into the market, and see how they fare against a backdrop of market makers, retail investors, and even high-capital "whale" traders.

It's built to be both realistic and fast, using modern, concurrent Rust to handle thousands of events per second.

## Key Features

*   **Realistic Exchange:** A true-to-life limit order book with price-time priority matching, partial fills, and cancellations.
*   **Concurrent Architecture:** The market and every agent run in their own threads, communicating through high-speed channels for maximum performance and realism.
*   **A World of Agents:** The simulation is populated by different types of traders, each with its own behavior, from simple market-order agents to sophisticated market makers.
*   **Pluggable Agent Framework:** It's easy to create your own trading agents. Just implement the `Agent` trait and drop your creation into the simulation.
*   **Live Visualization:** Comes with two real-time GUI applications built with `egui` to watch the market unfold. See the order book depth, price history, and more, live as it happens.
*   **Sentiment Analysis:** The simulation includes a sentiment analysis engine that can influence agent behavior, adding another layer of realism.
*   **Built with Rust:** All the performance, safety, and concurrency that Rust is famous for.

## How It Works: The Simulation Lifecycle

The simulation is managed by a central `Orchestra`, which is responsible for setting up and running the entire show.

1.  **Setup:** The `Orchestra` initializes the `Market` (the exchange), a cast of `Agents` (the traders), and the communication channels that connect them.
2.  **Launch:** Each agent and the market are spawned into their own native threads.
3.  **Interaction:** Agents make decisions based on their internal logic and a read-only "shadow" view of the market's order books. They send `OrderRequest` messages (limit orders, market orders, cancellations) to the market over a shared channel.
4.  **Matching:** The `Market` thread continuously processes incoming orders, matching them in its private order books and generating `Trade` events.
5.  **Communication:** The `Market` sends acknowledgements and trade notifications back to the relevant agents, who then update their internal state (like their cash and inventory).
6.  **Visualization:** While all this is happening, you can run one of the visualizer applications to get a live, real-time view of the market state.

## The Actors: A Look at the Agents

The simulation comes with a variety of built-in agent types:

*   **`DumbAgent`:** Represents the unpredictable nature of retail traders by placing random market orders.
*   **`DumbLimitAgent`:** A slightly more sophisticated retail trader that places limit orders around the current price.
*   **`MarketMakerAgent`:** Provides liquidity to the market by maintaining a tight bid-ask spread, ensuring there are always orders to trade against.
*   **`IpoAgent`:** Simulates an Initial Public Offering by placing a large number of sell orders at the beginning of the simulation.
*   **`WhaleAgent`:** A high-capital, patient trader that places large, infrequent limit orders far from the mid-price, representing institutional flow.

## Getting Started

### Prerequisites

*   Rust 1.60 or later
*   Cargo package manager

### Running the Simulation

1.  **Clone the repository:**

    ```bash
    git clone https://github.com/apoorvasinha183/market_simulator.git
    cd market_simulator
    ```

2.  **Build the project:**

    ```bash
    cargo build --release
    ```

3.  **Run the visualizer:**

    The best way to experience the simulation is through the live visualizer.

    ```bash
    cargo run --release --bin visual_order
    ```

    This will launch a GUI application where you can start, pause, and inspect the live order book and price history for the simulated stocks.

## Project Roadmap

| Phase | Description | Status |
| :--- | :--- | :--- |
| **Foundation** | Core architecture, multi-threaded orchestra, agent framework | ✅ Complete |
| **Core Actors** | Implementation of all major agent types | ✅ Complete |
| **Visualization** | Real-time order book and price history visualizer | ✅ Complete |
| **Testing** | Comprehensive unit and integration tests | ⏳ In Progress |
| **RL Integration** | A streaming interface (e.g., gRPC) for external policy control | ấp Planned |
| **Options Market** | Dedicated infrastructure for options pricing and trading | ấp Planned |

## Dependencies

The project leverages several high-quality Rust crates:

-   **eframe/egui**: For the modern GUI of the visualization tools.
-   **crossbeam-channel**: For high-performance, multi-threaded message passing.
-   **serde**: For data serialization.
-   **rand**: For random number generation.
-   **statrs**: For statistical functions and distributions.

## License

This project is licensed under the MIT License. A copy of the license can be found in the `LICENSE` file.
