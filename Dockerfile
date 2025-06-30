# Stage 1: Builder
FROM rust:latest AS builder

# Install protobuf compiler
RUN apt-get update && apt-get install -y protobuf-compiler

WORKDIR /usr/src/market_simulator

# Copy the entire project
COPY . .

# Build the visual_order binary
RUN cargo build --release --bin visual_order

# Stage 2: Final image
FROM debian:bookworm-slim

# Copy the binary from the builder stage
COPY --from=builder /usr/src/market_simulator/target/release/visual_order /usr/local/bin/visual_order

# Expose the gRPC port
EXPOSE 50051

# Set the default command to run the visualizer
CMD ["visual_order"]