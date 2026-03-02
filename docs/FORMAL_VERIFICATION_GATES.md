# Formal Verification Gates

## Why This Exists

ETH2077 will not use "formal verification" as a slogan. This document defines when the claim is allowed.

## Claim Tiers

### Tier 0: Research Prototype

Allowed:
- exploratory benchmarks,
- design hypotheses,
- non-production experiments.

Not allowed:
- production security claims,
- marketing language implying complete formal verification.

### Tier 1: Verified Consensus Core

Required:
- critical consensus modules have zero `sorry`, zero `axiom`,
- machine-checked proofs for safety and non-equivocation properties,
- adversarial tests aligned to theorem assumptions.

### Tier 2: Verified Execution Bridge

Required:
- proofs linking out-of-band commitments to canonical Ethereum execution validity,
- deterministic replay proofs for block import paths,
- differential tests against reference clients.

### Tier 3: Verified ETH2077 Client

Required:
- all critical-path modules satisfy no-placeholder policy,
- formal obligations pass in CI,
- benchmark claims reproducible with signed artifacts,
- clear threat model and assumption ledger published.

Only at Tier 3 can we claim "formally verified Ethereum client".

## Mandatory Theorem Families

1. Consensus safety (no conflicting finalized histories).
2. Consensus liveness under explicit network/fault assumptions.
3. Deterministic ordering and replay.
4. OOB-to-execution bridge soundness.
5. Witness/data availability integrity.
6. Crash/recovery invariants.
7. Anti-reordering and anti-equivocation guarantees.

## Benchmark Claim Policy

No TPS/finality claim may be published unless:

1. Build provenance is recorded.
2. Workload generator and seed are fixed.
3. Hardware/network profile is recorded.
4. Raw outputs are archived.
5. Corresponding proof gate status is attached.

## Current Baseline Debt (2026-03-02)

Measured from existing source proof programs:

- `citadel`: `sorry=10`, `axiom=78`
- `lagoon`: `sorry=90`, `axiom=15`

Interpretation:
- Both programs contain valuable formal structure.
- Neither currently satisfies Tier 3 critical-path no-placeholder requirements.

## Enforcement

- Use `scripts/check_eth2077_formal_gates.sh` for ETH2077 local proofs.
- Use `scripts/proof_debt_audit.sh` to track external baseline progress during migration.
