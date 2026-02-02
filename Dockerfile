FROM rust:1.85-slim

WORKDIR /app

# Pre-copy manifests for better caching
COPY Cargo.toml ./
COPY crates/sim_core/Cargo.toml crates/sim_core/Cargo.toml

# Create a minimal src to allow dependency fetch
RUN mkdir -p crates/sim_core/src && echo "pub mod spatial;" > crates/sim_core/src/lib.rs
RUN cargo fetch

# Copy the full workspace
COPY . .

# Default command runs tests
CMD ["cargo", "test", "--workspace"]
