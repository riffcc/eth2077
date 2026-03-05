# ETH2077

ETH2077 is the Citadel-based Ethereum client program focused on two non-negotiables:

1. Orders-of-magnitude scaling beyond current roadmap targets.
2. Formal verification strong enough that performance claims are backed by proofs and reproducible benchmarks.

## Mission

Build the first Ethereum client where high-scale claims are gated by theorem checks and empirical evidence, not narrative.

## Strategic Targets

- `L1 throughput`: sustain and verify `>= 1,000,000 TPS` in production-grade benchmark environments.
- `Finality`: deterministic finality in sub-5s target envelope, with explicit safety proofs.
- `Node accessibility`: keep low-cost/stateless operation as a first-class design objective.
- `Ethereum compatibility`: preserve canonical execution correctness and standard API compatibility.

## Principles

- No scaling claim without benchmark artifacts.
- No safety/liveness claim without machine-checked proofs.
- No "formally verified" claim while critical-path `axiom`/`sorry` debt remains.
- Optimize for real operator constraints: bandwidth, memory, CPU locality, failure recovery.

## Implementation Stack

- Runtime/client code: `Rust`.
- Formal verification lane: `Lean 4`.

## Quick Start

### Prerequisites

- Rust 1.83+ (`rustup update stable`)
- Or Docker / Docker Compose

### Build from source

```bash
cargo build --release -p eth2077-node --bin eth2077-devnet
```

### Run a single devnet node

```bash
# Defaults: chain ID 2077, RPC on :8545, P2P on :30303, 2s blocks
./target/release/eth2077-devnet

# Override via environment variables
PEER_ID=0 RPC_PORT=8545 BLOCK_TIME_MS=1000 ./target/release/eth2077-devnet
```

### Run with Docker

```bash
docker build -t eth2077-devnet .
docker run -p 8545:8545 -p 30303:30303 eth2077-devnet
```

### Run a 3-node cluster with Docker Compose

```bash
docker compose up --build
# Node 0 RPC → localhost:8545
# Node 1 RPC → localhost:8546
# Node 2 RPC → localhost:8547
```

### Interact via JSON-RPC

```bash
# Chain ID
curl -s -X POST http://localhost:8545 \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","method":"eth_chainId","params":[],"id":1}'

# Latest block number
curl -s -X POST http://localhost:8545 \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'

# Get balance (pre-funded test account)
curl -s -X POST http://localhost:8545 \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","method":"eth_getBalance","params":["0x1111111111111111111111111111111111111111","latest"],"id":1}'
```

### Environment Variables

| Variable | Default | Description |
|---|---|---|
| `PEER_ID` | `0` | Validator index for this node |
| `LISTEN_PORT` | `30303` | P2P listen port |
| `RPC_PORT` | `8545` | JSON-RPC server port |
| `BLOCK_TIME_MS` | `2000` | Target block interval in milliseconds |
| `CHAIN_ID` | `2077` | EVM chain ID |
| `BOOT_PEERS` | *(none)* | Comma-separated peer addresses (e.g. `10.0.0.2:30303`) |

### Run the integration test

```bash
cargo test --test gate4_devnet -p eth2077-node
```

## Workspace Crates

| Crate | Description |
|---|---|
| `eth2077-types` | Canonical block, header, transaction, and receipt types |
| `eth2077-execution` | EVM execution via revm, block builder, genesis, state DB |
| `eth2077-oob-consensus` | Out-of-band BFT consensus engine with Citadel fast-path finality |
| `eth2077-bridge` | JSON-RPC server (eth_ namespace), Engine API stubs |
| `eth2077-p2p` | Peer-to-peer networking: block/tx/consensus message gossip |
| `eth2077-node` | Devnet binary tying all layers together |
| `eth2077-bench` | Deterministic benchmark suite |
| `eth2077-live-bench` | Live network benchmarking |
| `eth2077-testnet` | Testnet tooling |

## Current Focus

- Working single-node devnet producing blocks with EVM execution.
- Out-of-band consensus with fast-path finality (single-validator mode operational).
- JSON-RPC compatibility: `eth_chainId`, `eth_blockNumber`, `eth_getBalance`, `eth_getBlockByNumber`, `eth_getTransactionReceipt`, `eth_getTransactionCount`, `eth_gasPrice`.
- Multi-node P2P devnet with block and consensus message gossip.

## Documents

- `docs/ETH2077_ARCHITECTURE.md`
- `docs/DEVNET.md`
- `docs/BLOCKSCOUT_DEVNET.md`
- `docs/INVESTOR_DEMO_MODE.md`
- `docs/NETWORK_OBSERVATORY.md`
- `docs/FORMAL_VERIFICATION_GATES.md`
- `docs/ROADMAP.md`
- `docs/STRAWMAP_DEMOLITION_MATRIX.md`
- `docs/TESTNET_LAUNCH_ALPHA.md`
- `reports/baseline-2026-03-02.md`

## Scripts

- `scripts/proof_debt_audit.sh` - measures external proof-debt baseline (`citadel`, `lagoon`).
- `scripts/check_eth2077_formal_gates.sh` - enforces ETH2077 local formal proof policy.
- `scripts/build_testnet_artifacts.sh` - generates deterministic testnet chain artifacts + checksums.
- `scripts/check_testnet_go_nogo.sh` - runs formal gates, benchmark gates, and testnet artifact gates.
- `scripts/devnet_up.sh` - deploys a local multi-node ETH2077 devnet.
- `scripts/devnet_status.sh` - queries live devnet node status.
- `scripts/devnet_down.sh` - stops local devnet processes.
- `scripts/blockscout_up.sh` - deploys Blockscout against local ETH2077 devnet.
- `scripts/blockscout_down.sh` - stops Blockscout stack.
- `scripts/investor_demo_mode.sh` - one-command 5-minute interactive demo prep + proof checklist.

## Benchmark Commands

- Default deterministic suite:
  - `cargo run -p eth2077-bench --release -- --scenario-set default --seed 2077 --tx-count 600000`
- 48-node bottleneck sensitivity sweep:
  - `cargo run -p eth2077-bench --release -- --scenario-set bottleneck-48n --seed 2077 --tx-count 600000`
