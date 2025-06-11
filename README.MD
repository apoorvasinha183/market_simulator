# Market Simulator

A high-performance financial market simulator in Rust, designed for developing and testing algorithmic trading agents, particularly those using Reinforcement Learning.

## About This Project

This sandbox is built from the ground up in Rust for maximum performance and safety. It allows for the generation of fictitious, yet realistic, time series data that agents can interact with. The core simulation engine is designed as a library, making it easy to integrate with other tools and frameworks in the future. 

## Current Phase

- **Phase 0: Foundation** - Project setup, structure, and GitHub integration. (Complete)
- **Phase 1: Single Asset Simulation** - Implementing Geometric Brownian Motion (GBM). (Complete)
- **Phase 2: Single Asset Simulation Option Pricing** - Black Scholes Model to estimate Fair Option Pricing  (Complete)
- **Phase 3: Order Book Dynamics** - An interative order book level price generation  (Complete)
- **Phase 4: Modelling market participants** - Modelling market forces as an ensemble (In Progress)

## Building

To build the library, run the following command:
```bash
cargo build --release