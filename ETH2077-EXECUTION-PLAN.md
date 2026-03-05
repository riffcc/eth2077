# ETH2077 Execution Plan: From Scaffolding to Working Client

**Status:** Architecture complete, scaffolding compiled clean, zero production logic
**Date:** 2026-03-05
**Author:** Wings + Claude

---

## Honest Assessment

ETH2077 today is **~5-8% of a working Ethereum client**. What exists:

- 49,000 lines of Rust across 8 crates, 179 files
- Complete type scaffolding for ~100 EIP/feature areas (all compile clean)
- Working deterministic mesh performance simulator (eth2077-bench)
- Consensus fast-path attestation accumulator with quorum logic
- Mempool/ingress with eviction logic
- Chain spec validation with SHA-256 commitment
- A devnetd binary integrating `revm` for EVM execution (same approach as Reth)

What's completely missing:

- No Merkle Patricia Trie / state management
- No P2P networking (devp2p/libp2p)
- No sync algorithms
- No real block validation against consensus rules
- No persistent storage layer
- No cryptographic verification pipeline
- No JSON-RPC API beyond basic devnetd wrapper

The codebase reads as **strong architecture + type scaffolding + benchmark harness**. The Citadel-style OOB consensus is the unique differentiator, but it's only a fast-path accumulator today.

---

## Strategic Decision: What Is ETH2077?

Before writing more code, we need to pick a lane. Three options:

### Option A: Research Prototype (Current trajectory)
Keep the type scaffolding, benchmarks, and architecture docs as an opinionated vision document for Ethereum's future. Publish as a "what if" design artifact. **No further implementation needed.**

### Option B: Execution Layer Client (Reth/Geth competitor)
Build a full execution client that can sync mainnet, validate blocks, serve JSON-RPC. This is 50,000-200,000 lines of production Rust and 12-24 months of focused engineering even with AI assistance. (EVM execution via revm — same as Reth — so that's not the bottleneck; it's everything else.)

### Option C: Modular Execution Extension (Recommended)
Keep `revm` for EVM execution. Build the *differentiating* pieces: OOB consensus layer, Citadel fast-path finality, the bridge layer to existing CL clients, and a working devnet. This is achievable in 3-6 months and produces something demonstrably novel.

**This plan assumes Option C** unless Wings decides otherwise.

---

## Milestone Gates

### Gate 0: Foundation (Current → Week 2)
**Goal:** Real state management and block structure

| Task | Crate | Est. Lines | Priority |
|------|-------|-----------|----------|
| Integrate `alloy-trie` for Merkle Patricia Trie | eth2077-types | 500 | P0 |
| Define canonical Block, Header, Transaction types using alloy primitives | eth2077-types | 800 | P0 |
| Implement RLP encode/decode for all core types (via alloy-rlp) | eth2077-types | 400 | P0 |
| Build InMemoryStateDB with account storage, nonce, balance, code | eth2077-execution | 1,200 | P0 |
| Wire revm to InMemoryStateDB for real EVM execution | eth2077-execution | 600 | P0 |
| Add `alloy`, `alloy-trie`, `alloy-rlp`, `alloy-primitives` to workspace deps | Cargo.toml | 20 | P0 |

**Gate criteria:** Execute a simple ETH transfer against InMemoryStateDB, verify state root changes.

### Gate 1: Block Pipeline (Week 2 → Week 4)
**Goal:** Build and validate blocks end-to-end

| Task | Crate | Est. Lines | Priority |
|------|-------|-----------|----------|
| Block builder: order transactions, compute state root, gas accounting | eth2077-execution | 1,500 | P0 |
| Block validator: verify header, replay transactions, check state root | eth2077-execution | 1,200 | P0 |
| Transaction signature verification (secp256k1 via alloy) | eth2077-execution | 300 | P0 |
| Receipt/log generation and bloom filter computation | eth2077-execution | 500 | P1 |
| Genesis block creation from chain spec | eth2077-node | 400 | P0 |

**Gate criteria:** Build a block from pending transactions, validate it, produce correct state root. Round-trip a 10-block chain deterministically.

### Gate 2: OOB Consensus (Week 4 → Week 8)
**Goal:** The differentiator — working Citadel-style consensus

| Task | Crate | Est. Lines | Priority |
|------|-------|-----------|----------|
| Full BFT voting protocol (not just fast-path accumulator) | eth2077-oob-consensus | 2,000 | P0 |
| View change / leader election | eth2077-oob-consensus | 800 | P0 |
| Finality gadget with 1-round optimistic + 2-round fallback | eth2077-oob-consensus | 1,000 | P0 |
| Validator set management (join/leave/slash) | eth2077-oob-consensus | 600 | P1 |
| Consensus-execution interface (ConsensusEngine trait → real impl) | eth2077-bridge | 800 | P0 |
| Integration tests: 4-node BFT with one Byzantine | eth2077-oob-consensus | 1,500 | P0 |

**Gate criteria:** 4 in-process consensus nodes agree on a chain of 100 blocks with one simulated Byzantine node. Finality latency < 2 seconds in test.

### Gate 3: Networking (Week 8 → Week 12)
**Goal:** Nodes can find each other and exchange blocks

| Task | Crate | Est. Lines | Priority |
|------|-------|-----------|----------|
| libp2p transport layer (TCP + noise + yamux) | eth2077-node | 1,500 | P0 |
| Block announcement / request protocol | eth2077-node | 800 | P0 |
| Transaction gossip protocol | eth2077-node | 600 | P0 |
| Consensus message transport | eth2077-node | 500 | P0 |
| Peer discovery (mDNS for devnet, Kademlia for later) | eth2077-node | 400 | P1 |
| Basic sync: request missing blocks from peers | eth2077-node | 1,000 | P0 |

**Gate criteria:** 3 nodes on localhost discover each other, sync a chain from genesis, reach consensus on new blocks.

### Gate 4: JSON-RPC & Devnet (Week 12 → Week 16)
**Goal:** External tools can interact with the node

| Task | Crate | Est. Lines | Priority |
|------|-------|-----------|----------|
| JSON-RPC server (eth_blockNumber, eth_getBalance, eth_sendRawTransaction, eth_call) | eth2077-node | 1,500 | P0 |
| Engine API bridge for CL client compatibility | eth2077-bridge | 1,200 | P1 |
| Persistent storage (sled or rocksdb) for blocks + state | eth2077-node | 1,000 | P0 |
| Devnet launcher: spawn N nodes, fund accounts, run workload | eth2077-testnet | 800 | P0 |
| Metrics / Prometheus endpoint | eth2077-node | 300 | P2 |

**Gate criteria:** Deploy a 4-node devnet. Send transactions via `cast` (foundry). Blocks produced and finalized. State queryable via JSON-RPC.

### Gate 5: Hardening & Public Devnet (Week 16 → Week 24)
**Goal:** Other people can run it

| Task | Crate | Est. Lines | Priority |
|------|-------|-----------|----------|
| Fuzz testing (block builder, consensus, RPC) | all | 2,000 | P0 |
| Crash recovery (WAL for consensus, state snapshots) | eth2077-node | 1,500 | P0 |
| Docker packaging | infra | 200 | P1 |
| Documentation: architecture, running a node, contributing | docs | 2,000 | P1 |
| Public devnet with faucet | eth2077-testnet | 500 | P1 |
| Security audit prep (threat model review) | docs | 1,000 | P2 |

**Gate criteria:** Public devnet running for 7 days without intervention. External users can connect, send transactions, query state.

---

## Dependency Strategy

Stay in the alloy ecosystem for Ethereum primitives. Don't reinvent:

| Need | Crate | Rationale |
|------|-------|-----------|
| EVM execution | `revm` | Already integrated. Same choice as Reth — production-grade Rust EVM. |
| Ethereum types | `alloy-primitives` | B256, U256, Address, Bloom — standard |
| RLP encoding | `alloy-rlp` | Derive macros, well-tested |
| State trie | `alloy-trie` | MPT implementation |
| Crypto (secp256k1) | `alloy-signer` | Signature verification |
| Networking | `libp2p` | Rust-native, modular |
| Storage | `sled` or `redb` | Embedded, no external deps |
| JSON-RPC | `jsonrpsee` | Async, well-maintained |
| Async runtime | `tokio` | Already in workspace |

---

## Lines of Code Estimate

| Gate | New Lines | Cumulative | % of Target |
|------|-----------|-----------|-------------|
| Current | 49,000 (scaffolding) | 49,000 | 5-8% |
| Gate 0: Foundation | ~3,500 | 52,500 | 15% |
| Gate 1: Block Pipeline | ~3,900 | 56,400 | 25% |
| Gate 2: OOB Consensus | ~6,700 | 63,100 | 40% |
| Gate 3: Networking | ~4,800 | 67,900 | 55% |
| Gate 4: JSON-RPC & Devnet | ~4,800 | 72,700 | 70% |
| Gate 5: Hardening | ~7,200 | 79,900 | 85% |

The remaining 15% is iteration, edge cases, and things we'll discover along the way. A working modular Ethereum node with novel OOB consensus at ~80K lines is realistic — Reth is ~200K but does everything natively including EVM.

---

## Codex Sprint Strategy

Each gate maps to 15-25 Plane tickets. The pattern that worked for scaffolding (parallel non-conflicting Codex sessions) applies, but with a key difference: **Gate 0-2 tasks have real data dependencies.** We can't parallelize "build state DB" and "wire revm to state DB" — they're sequential.

Parallelism opportunities per gate:

- **Gate 0:** Types (alloy migration) ∥ StateDB skeleton ∥ Cargo.toml deps — then wire together
- **Gate 1:** Block builder ∥ Signature verification ∥ Receipt generation — then validator integrates all
- **Gate 2:** BFT core ∥ View change ∥ Validator management — then integration tests
- **Gate 3:** Transport ∥ Gossip protocol ∥ Discovery — then sync wires them together
- **Gate 4:** RPC server ∥ Storage layer ∥ Devnet launcher — then Engine API bridges

Estimate: 2-3 Codex rounds per gate, with manual review between rounds.

---

## Risk Register

| Risk | Impact | Mitigation |
|------|--------|------------|
| revm API changes between versions | High | Pin exact version, wrap in thin adapter layer |
| OOB consensus has liveness bugs under partition | Critical | Extensive simulation before networking |
| State trie performance at scale | Medium | Start with alloy-trie, profile, optimize later |
| libp2p complexity / breaking changes | Medium | Use stable release, minimal protocol surface |
| Codex generates plausible but incorrect consensus logic | Critical | Every consensus change gets formal review + simulation |

---

## Next Action

**Start Gate 0.** The first concrete ticket: add `alloy-primitives`, `alloy-rlp`, and `alloy-trie` to workspace dependencies, then define canonical `Block`, `Header`, `Transaction` types using alloy primitives instead of the current ad-hoc scaffolding types.
