# ETH2077 EIP-7732 Implementation Plan

Date: 2026-03-03

## Current Progress

- 2026-03-03 (Phase 1 started and landed in devnetd):
  - slot-indexed header/envelope stores added.
  - engine endpoints added for header/envelope registration, envelope lookup, and timeliness status.
  - baseline status taxonomy exposed: `UNKNOWN`, `HEADER_ONLY`, `PARTIAL_REVEAL`, `REVEALED`, `LATE_REVEAL`, `ORPHAN_ENVELOPE`.
- 2026-03-03 (Phase 2 initial enforcement landed in devnetd):
  - deadline-aware withheld states added: `WITHHELD` and `PARTIAL_WITHHOLD`.
  - forkchoice rejects slots with `WITHHELD`/`PARTIAL_WITHHOLD`/`LATE_REVEAL` timeliness violations.
  - deterministic test hook (`currentUnixS`) added for reproducible interval/deadline behavior.
- 2026-03-03 (Phase 2 extension landed in devnetd):
  - `engine_newPayloadV3` now rejects `WITHHELD`/`PARTIAL_WITHHOLD`/`LATE_REVEAL` slots as `INVALID`.
  - slot-local penalty lifecycle added with explicit `ACTIVE` -> `RECOVERED` transitions.
  - timeliness and penalty snapshots now returned in forkchoice/newPayload responses.
- 2026-03-03 (Phase 2 replay hardening landed in devnetd):
  - header/envelope registration is now replay-aware: idempotent duplicates are accepted, conflicting replays are rejected.
  - envelope registration now rejects slot mismatch against known header root commitments.
  - tests added for duplicate, conflict, and mismatch rejection paths.

## Primary Sources

- EIP-7732 beacon-chain feature spec:
  - https://ethereum.github.io/consensus-specs/specs/_features/eip7732/beacon-chain/
- EIP-7732 honest-validator duties:
  - https://ethereum.github.io/consensus-specs/specs/_features/eip7732/validator/
- EIP-7732 networking surface:
  - https://ethereum.github.io/consensus-specs/specs/_features/eip7732/p2p-interface/
- EIP-7732 builder duties:
  - https://ethereum.github.io/consensus-specs/specs/_features/eip7732/builder/

## ETH2077 Scope (Execution-Client Side)

EIP-7732 is consensus-heavy, but ETH2077 needs EL/runtime hooks for:

1. Bid/header commitment tracking
- Capture `SignedExecutionPayloadHeader`-equivalent commitments.
- Maintain parent-hash/root linkage and slot-indexed commitment store.

2. Envelope reveal/withhold path
- Accept `SignedExecutionPayloadEnvelope`-equivalent reveals.
- Record reveal timeliness state and payload-available status.

3. Payload timeliness signals
- Track PTC-like attestations and aggregate status per slot.
- Expose status in engine-facing surfaces for fork-choice consumers.

4. Recovery and transport hooks
- Add request/response hooks for payload-envelope retrieval by root/range.
- Keep this deterministic in devnet harness before full p2p integration.

## Delivery Phases

1. Data model + RPC scaffold
- Add slot-indexed header/envelope stores.
- Add JSON-RPC/HTTP endpoints for commitment/reveal and status query.

2. Timeliness state machine
- Implement interval-aware slot deadlines.
- Add payload status transitions: `unknown -> revealed|withheld|late`.

3. Fork-choice integration
- Feed payload status into block/slot validity gate in devnetd.
- Add explicit rejection/penalty simulation hooks for late/missing reveals.

4. Test and proof gates
- Runtime tests for happy path, withheld payload, late reveal, and replay.
- Lean proof obligations for monotonic status transitions and slot-local immutability.

## Acceptance Gates

- Runtime:
  - deterministic tests pass for header commit + envelope reveal lifecycle.
  - late/withheld payload states are observable and enforced.
- Proof:
  - Lean obligations for status monotonicity and no illegal back-transitions.
- Bench:
  - no TPS/finality regression versus current EIP-7805 lane baseline.
