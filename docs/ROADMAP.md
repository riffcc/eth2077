# ETH2077 Roadmap (Execution Plan)

## Phase 0 - Baseline and Gate Installation (current)

Deliverables:
- architecture draft,
- formal verification gate policy,
- proof-debt baseline report,
- scriptable gate checks.

Exit criteria:
- baseline report checked in,
- gate scripts runnable locally.

## Phase 1 - Ethereum Compatibility Skeleton

Deliverables:
- execution client skeleton with Ethereum-compatible interfaces,
- out-of-band consensus adapter interface,
- deterministic block pipeline skeleton.

Exit criteria:
- compatibility harness runs against reference vectors,
- CI proves deterministic replay on sample traces.

## Phase 2 - OOB Consensus Integration

Deliverables:
- Citadel-derived ordering/finality adapter,
- conflict-resolution and commitment bridge,
- failure-mode and recovery logic.

Exit criteria:
- safety/liveness theorem family v1 passes,
- no placeholder proofs in OOB critical modules.

## Phase 3 - Witness and State Diff Scaling

Deliverables:
- SPORE/transfer-based witness diff subsystem,
- stateless-first node sync path,
- data integrity verification pipeline.

Exit criteria:
- integrity and anti-corruption proofs pass,
- bandwidth and latency benchmarks meet phase targets.

## Phase 4 - Performance Program (toward 1M L1 TPS)

Deliverables:
- deterministic high-throughput benchmark harness,
- parallel execution and commit pipelining,
- bottleneck profiling and optimization loops.

Exit criteria:
- staged throughput targets met with reproducible artifacts,
- performance claims linked to passing formal gate status.

## Phase 5 - Hardening and Security Proof Closure

Deliverables:
- adversarial networking test suite,
- crash/recovery model verification,
- cryptographic path hardening.

Exit criteria:
- zero critical `sorry`/`axiom` debt,
- threat model assumptions published and validated.

## Phase 6 - Verified Client Claim

Deliverables:
- complete verification report,
- reproducible benchmark dossier,
- release candidate profile.

Exit criteria:
- Tier 3 formal gate satisfied,
- claim package ready: "formally verified Ethereum client".

## Near-Term Sprint (next)

1. Build ETH2077 crate/workspace skeleton.
2. Port first critical Citadel module with placeholder removal plan.
3. Define theorem-by-theorem acceptance checklist tied to CI jobs.
4. Stand up deterministic benchmark harness with fixed seeds.
5. Execute EIP parity work one EIP at a time using `docs/EIP_EXECUTION_MATRIX.md` (EIP-4844/7702 runtime lanes complete; next is sidecar/DA + conformance depth).
