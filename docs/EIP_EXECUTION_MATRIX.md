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
| EIP-7805 | FOCIL inclusion-list engine lane | partial | Engine API lane now includes `engine_getInclusionListV1` plus `engine_forkchoiceUpdatedV3`/`engine_newPayloadV3` inclusion-list wiring, metadata compatibility checks (sender/nonce/gas/value), and basic nonce/balance/gas-space enforcement with `INCLUSION_LIST_UNSATISFIED`; CL/P2P committee logic still pending. |

## Execution Order (One EIP At A Time)

1. EIP-4844 completion:
   - implement blob sidecar/DA plumbing and validation path.
   - map blob economics into benchmark scenarios and status surfaces.
2. EIP-7702 completion:
   - add conformance tests for invalid signatures/chain-id mismatch/replay semantics.
   - align authorization handling with stricter recovery/validation expectations.
3. EIP-7805 completion:
   - wire CL/P2P committee and view-freeze semantics.
   - extend IL validity checks from heuristic precheck to full execution-congruent validation.
4. EIP-7685 and other roadmap-targeted request/interop features.

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
