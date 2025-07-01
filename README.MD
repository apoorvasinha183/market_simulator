# Market Simulation Platform

This project provides a high-performance, distributed financial market simulator written in Rust. It is designed for research, testing, and analysis of trading strategies and market dynamics within a realistic limit order book environment. The platform enables the simulation of various market participants and their interactions, offering a robust tool for understanding complex market behaviors.

## Key Features

*   **High-Performance Matching Engine:** Implements a true-to-life limit order book with price-time priority matching, partial fills, and cancellations, optimized for speed and concurrency in Rust.
*   **Multi-threaded Agent Simulation:** The market and each agent operate in their own threads, communicating through high-speed channels for maximum performance and realism.
*   **Diverse Agent Ecosystem:** Populated by various agent types, each with distinct behaviors, including market order agents, sophisticated market makers, and high-capital "whale" traders.
*   **Pluggable Agent Framework:** An extensible `Agent` trait in Rust allows for easy development and integration of custom trading agents.
*   **Real-time Communication via gRPC:** Utilizes gRPC for efficient, bi-directional streaming of market data and order flow between the Rust backend and Python clients, enabling external control and observation.
*   **Configurable Market Parameters:** Provides extensive configuration options for tuning agent behaviors, liquidity levels, and other market parameters to simulate diverse market conditions.
*   **Live Visualization:** Includes real-time GUI applications built with `egui` to observe market dynamics, order book depth, and price history as the simulation unfolds.
*   **Sentiment Analysis Integration:** Features a sentiment analysis engine that can influence agent behavior, adding an additional layer of market realism.
*   **Docker Support:** Comprehensive Docker configurations are provided for streamlined setup and deployment of the entire simulation environment.

## Architecture

The platform employs a client-server architecture with a clear separation of concerns:

*   **Rust Backend:** This is the core of the simulation, implemented in Rust for its performance and concurrency capabilities. It manages the central `Market` (the exchange), which handles order books, trade execution, and agent state updates. The backend also hosts the gRPC server, exposing the simulation's API.
*   **Python Client:** This component serves as the external interface to the simulation. It connects to the Rust gRPC server, allowing users to send various order types (Market, Limit) and receive real-time updates such as order acknowledgments and trade notifications. This enables flexible control and data consumption for external trading algorithms or analysis tools.
*   **gRPC Communication:** Protocol Buffers define the service contract (`market_gateway.proto`), ensuring a language-agnostic and efficient communication layer. A bi-directional streaming RPC facilitates real-time data exchange between the Rust backend and Python clients.

## Technologies Used

*   **Rust:**
    *   `tokio`: Asynchronous runtime for concurrent operations.
    *   `tonic`, `prost`: gRPC framework and Protocol Buffer implementation.
    *   `crossbeam-channel`: High-performance multi-producer, multi-consumer channels.
    *   `parking_lot`, `once_cell`: Concurrency primitives for shared state management.
    *   `eframe`, `egui_plot`: For GUI development and plotting in the visualizer.
    *   `criterion`: Benchmarking framework.
*   **Python:**
    *   `grpcio`, `grpcio-tools`: Python gRPC libraries.
    *   `pytest`: For testing the Python client.
*   **Docker:** For containerization and simplified deployment using `docker-compose`.

## Setup and Installation

### Prerequisites

*   Rust (with Cargo)
*   Python 3.10+
*   Conda (recommended for Python environment management)
*   Docker (if using Docker for deployment)

### Local Setup (without Docker)

1.  **Clone the repository:**
    ```bash
    git clone https://github.com/apoorvasinha183/market_simulator.git
    cd market_simulator
    ```

2.  **Rust Backend Setup:**
    *   Build the Rust project:
        ```bash
        cargo build --release
        ```
    *   Run the gRPC server:
        ```bash
        cargo run --release --bin grpc_server
        ```
        Keep this terminal open as the server will run continuously.

3.  **Python Client Setup:**
    *   Navigate to the `python_client` directory:
        ```bash
        cd python_client
        ```
    *   Create and activate the Conda environment:
        ```bash
        conda env create -f environment.yml
        conda activate market_simulator_env
        ```
    *   Install Python dependencies:
        ```bash
        pip install -r requirements.txt
        ```
    *   Generate gRPC Python stubs (if not already generated or if `.proto` files change):
        ```bash
        python -m grpc_tools.protoc -I../proto --python_out=./market_gateway_client/generated --grpc_python_out=./market_gateway_client/generated ../proto/market_gateway.proto
        ```
    *   Run the Python broker client:
        ```bash
        python run_broker.py
        ```
        This will connect to the Rust server and begin sending orders.

### Docker Setup

1.  **Build and run services using Docker Compose:**
    ```bash
    docker-compose up --build
    ```
    This command will build the Docker images for both the Rust server and Python client, and then start them.

## Usage

Once the Rust gRPC server and Python client are running:

*   The Rust server's console will display market events, agent actions, and trade executions.
*   The Python client's console will show the orders being sent and any acknowledgments or trade updates received from the server.
*   To run the live market visualizer:
    ```bash
    cargo run --release --bin visual_order
    ```
    This will launch a GUI application providing a real-time view of the order book and price history.

## Project Structure

*   `./`: Root directory, containing `Cargo.toml`, `docker-compose.yml`, `README.md`, etc.
*   `proto/`: Defines the gRPC service contract in `market_gateway.proto`.
*   `src/`: Rust source code for the simulation backend.
    *   `src/agents/`: Implementations of various agent behaviors and configurations.
    *   `src/bin/`: Executable binaries, including the gRPC server and visualizer.
    *   `src/pricing/`: Modules for pricing models.
    *   `src/sentiment_engine/`: Components for sentiment analysis.
    *   `src/simulation/`: Core orchestration logic for the simulation.
    *   `src/simulators/`: Implementations of the market and order book.
    *   `src/stocks/`: Definitions and management of stock instruments.
    *   `src/types/`: Fundamental data structures for orders, trades, and other market entities.
*   `python_client/`: Python client application.
    *   `python_client/market_gateway_client/`: Python client logic and generated gRPC files.
    *   `python_client/tests/`: Unit and integration tests for the Python client.
    *   `python_client/environment.yml`, `python_client/requirements.txt`: Python dependency management files.
*   `benches/`: Rust benchmarking code.
*   `tests/`: Rust integration tests.

## Project Roadmap

| Phase | Description | Status |
| :--- | :--- | :--- |
| **Foundation** | Core architecture, multi-threaded orchestra, agent framework | Complete |
| **Core Actors** | Implementation of all major agent types | Complete |
| **Visualization** | Real-time order book and price history visualizer | Complete |
| **Testing** | Comprehensive unit and integration tests | In Progress |
| **RL Integration** | A streaming interface (e.g., gRPC) for external policy control | Planned |
| **Options Market** | Dedicated infrastructure for options pricing and trading | Planned |

## License

This project is licensed under the MIT License. See the `LICENSE` file for details.