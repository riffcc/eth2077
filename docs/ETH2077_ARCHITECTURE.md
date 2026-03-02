# ETH2077 Architecture (Draft v0)

## Objective

ETH2077 is an Ethereum client architecture that uses Citadel-style out-of-band consensus primitives to remove bottlenecks from the canonical execution path while preserving Ethereum correctness guarantees.

## Baseline Findings (2026-03-02)

### ETH2030 external benchmark target set

From public descriptions and roadmap framing:
- high-throughput L1 target,
- faster finality,
- lower validator capital requirements,
- stateless/low-cost node operation,
- very high aggregate L1+L2 throughput.

### Citadel and Lagoon reusable strengths

- Bilateral coordination protocol primitives (`citadel-protocols` coordinator path).
- SPORE-style diff sync and content exchange primitives (`citadel-spore`, `citadel-protocols::spore_sync`).
- High-throughput transfer scaffolding (`citadel-transfer`).
- Transport and mobility primitives (`anymesh`, Lagoon transport stack).
- Existing Lean proof programs that can be matured into strict no-placeholder formal assets.

### Current maturity blockers

- Proof debt is still material in source programs.
- Several transport/validation paths are placeholders or partially implemented.
- Ethereum-native execution client surfaces are not yet implemented in Citadel/Lagoon.

## High-Level ETH2077 Design

### 1. Execution Plane (Ethereum-canonical)

Responsibilities:
- transaction validation and execution,
- state transitions,
- block import/export,
- Engine API compatibility.

Constraint:
- canonical execution correctness must remain equivalent to Ethereum reference semantics.

### 2. Out-of-Band Consensus Plane (Citadel-derived)

Responsibilities:
- fast pre-finality agreement,
- resilient dissemination,
- topology- and latency-aware coordination,
- deterministic conflict resolution over candidate ordering.

Design rule:
- out-of-band consensus can accelerate ordering/finality paths, but cannot violate execution-plane correctness.

### 3. Witness + Data Diff Plane (SPORE/Transfer-derived)

Responsibilities:
- efficient state/witness distribution,
- bandwidth minimization under churn,
- deterministic replay/verification pathways.

### 4. Interop/Compatibility Plane

Responsibilities:
- standard Ethereum RPC compatibility,
- builder/proposer interoperability,
- phased rollout without chain forks caused by client idiosyncrasies.

## Core ETH2077 Invariants

1. Safety: no conflicting finalized blocks.
2. Liveness: progress under bounded adversarial assumptions.
3. Determinism: identical input produces identical ordered execution results.
4. Availability: degraded network states preserve eventual recovery.
5. Compatibility: execution outputs match Ethereum canonical behavior.

## Out-of-Band Finality Model (Draft)

- `Phase A`: execution plane builds candidate block bundles.
- `Phase B`: out-of-band consensus computes deterministic ordering commitments.
- `Phase C`: execution plane imports only commitments that satisfy proof-checked validity constraints.
- `Phase D`: finality witnesses are gossiped and compactly verifiable.

This split is the main lever for scaling while keeping Ethereum correctness intact.

## Gap-Closure Priorities

1. Replace placeholder cryptographic validation paths.
2. Eliminate proof placeholders (`sorry`) and unproven assumptions (`axiom`) on critical paths.
3. Implement Ethereum-native protocol/execution interfaces.
4. Build deterministic high-throughput benchmark harnesses with reproducible artifacts.
5. Add fault-injection and adversarial test suites tied to theorem obligations.
