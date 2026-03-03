# ETH2077 Execution EIP Matrix

Date: 2026-03-02

This matrix tracks execution-layer EIP parity work against the ETH2030 target surface.

## Status Keys

- `implemented`: supported in ETH2077 runtime path.
- `partial`: parser/runtime pieces exist but feature is incomplete.
- `missing`: not yet implemented.

## Current Coverage

| EIP | Scope | ETH2077 Status | Notes |
|---|---|---|---|
| EIP-2718 | Typed transaction envelope | implemented | Typed tx dispatch in raw tx decode path. |
| EIP-2930 | Access-list transaction (`0x01`) | implemented | Decode + execution path present. |
| EIP-1559 | Dynamic fee tx (`0x02`) | implemented | Decode + execution path present. |
| EIP-4844 | Blob transaction (`0x03`) | partial | Decode + execution path + runtime JSON-RPC tests + blob fee RPC fields are wired; no blob sidecar/DA plumbing yet. |
| EIP-7702 | Set-code transaction (`0x04`) | partial | Decode + execution path + runtime JSON-RPC tests are wired; conformance vectors and stricter auth semantics pending. |
| EIP-7805 | FOCIL inclusion-list engine lane | partial | Engine API lane now includes `engine_getInclusionListV1` plus `engine_forkchoiceUpdatedV3`/`engine_newPayloadV3` inclusion-list wiring, metadata compatibility checks (sender/nonce/gas/value), basic nonce/balance/gas-space enforcement, and server-side committee + view-freeze semantics (immutable IL within frozen slot) with `INCLUSION_LIST_UNSATISFIED`; full CL/P2P committee gossip/attestation path still pending. |
| EIP-7732 | ePBS consensus/EL integration lane | partial | Phase 1+2 scaffold landed in devnetd: slot-indexed header/envelope stores, engine registration endpoints, envelope lookup, timeliness status surface (`UNKNOWN`/`HEADER_ONLY`/`PARTIAL_REVEAL`/`REVEALED`/`LATE_REVEAL`/`WITHHELD`/`PARTIAL_WITHHOLD`/`ORPHAN_ENVELOPE`), rejection in both `engine_forkchoiceUpdatedV3` and `engine_newPayloadV3` for withheld/late slots, slot penalty lifecycle (`ACTIVE`/`RECOVERED`), and replay hardening (idempotent duplicate registration + conflicting replay rejection + header/envelope slot-match enforcement). Full CL+P2P integration and attestation wiring remain. |

## Execution Order (One EIP At A Time)

1. EIP-4844 completion:
   - implement blob sidecar/DA plumbing and validation path.
   - map blob economics into benchmark scenarios and status surfaces.
2. EIP-7702 completion:
   - add conformance tests for invalid signatures/chain-id mismatch/replay semantics.
   - align authorization handling with stricter recovery/validation expectations.
3. EIP-7805 completion:
   - wire CL/P2P committee gossip/attestation path to replace server-local committee scaffolding.
   - extend IL validity checks from heuristic precheck to full execution-congruent validation.
4. EIP-7685 and other roadmap-targeted request/interop features.
5. EIP-7732 phased integration per `docs/EIP_7732_IMPLEMENTATION_PLAN.md`.

## Required Evidence Per EIP

1. Parser tests:
   - positive decode tests for canonical fields.
   - negative decode tests for malformed payloads.
2. Runtime tests:
   - execute transaction through JSON-RPC entrypoint and verify receipt/type fields.
3. Formal gate:
   - no new placeholder debt in ETH2077 proof targets.
4. Benchmark gate:
   - no throughput/finality regression in deterministic suite.
