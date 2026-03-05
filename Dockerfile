# ETH2077 Devnet — multi-stage build
# Produces a minimal image with the eth2077-devnet binary.
#
# Build:  docker build -t eth2077-devnet .
# Run:    docker run -p 8545:8545 -p 30303:30303 eth2077-devnet
#
# Environment variables (all optional):
#   PEER_ID        — validator index (default 0)
#   LISTEN_PORT    — P2P listen port (default 30303)
#   RPC_PORT       — JSON-RPC port (default 8545)
#   BLOCK_TIME_MS  — target block interval in ms (default 2000)
#   CHAIN_ID       — chain ID (default 2077)
#   BOOT_PEERS     — comma-separated peer addresses (e.g. "10.0.0.2:30303,10.0.0.3:30303")

# ── Stage 1: build ──────────────────────────────────────────────
FROM rust:1.83-bookworm AS builder

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/

# Build only the devnet binary in release mode.
RUN cargo build --release -p eth2077-node --bin eth2077-devnet

# ── Stage 2: runtime ────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/eth2077-devnet /usr/local/bin/eth2077-devnet

# Default env — can be overridden at runtime.
ENV PEER_ID=0 \
    LISTEN_PORT=30303 \
    RPC_PORT=8545 \
    BLOCK_TIME_MS=2000 \
    CHAIN_ID=2077

EXPOSE 8545 30303

ENTRYPOINT ["eth2077-devnet"]
