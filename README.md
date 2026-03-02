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

## Current Focus

- Analyze ETH2030 claim architecture and gaps.
- Reuse mature primitives from `citadel` and `lagoon` where appropriate.
- Formalize ETH2077 obligations and install hard gates for proof debt and claim maturity.

## Documents

- `docs/ETH2077_ARCHITECTURE.md`
- `docs/FORMAL_VERIFICATION_GATES.md`
- `docs/ROADMAP.md`
- `docs/STRAWMAP_DEMOLITION_MATRIX.md`
- `reports/baseline-2026-03-02.md`

## Scripts

- `scripts/proof_debt_audit.sh` - measures external proof-debt baseline (`citadel`, `lagoon`).
- `scripts/check_eth2077_formal_gates.sh` - enforces ETH2077 local formal proof policy.
