# Stage 1: Builder
FROM rust:latest AS builder

# Install protobuf compiler
RUN apt-get update && apt-get install -y protobuf-compiler

WORKDIR /usr/src/market_simulator

# Copy the entire project
COPY . .

# Build the visual_order binary
RUN cargo build --release --bin grpc_server

# Stage 2: Final image
FROM debian:bookworm-slim

# Copy the binary from the builder stage
COPY --from=builder /usr/src/market_simulator/target/release/grpc_server /usr/local/bin/grpc_server

# Expose the gRPC port
EXPOSE 50051

# Set the default command to run the visualizer
CMD ["grpc_server"]