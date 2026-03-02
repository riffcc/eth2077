# Plane Decomposition Snapshot (ETH2077)

Date: 2026-03-02
Project: ETH2077 (`eea12bf6-00e4-4f7a-8803-de304e00a99f`)

## Summary

- Created 30 decomposition child issues (`ETH2077-12` .. `ETH2077-41`).
- Scope now has concrete implementation tickets under each major roadmap item.
- Attempted to move sprint starters to `In Progress`, but Plane MCP transport dropped (`Transport closed`).

## Created Child Issues

### Parent `ETH2077-2` (workspace)
- `ETH2077-12` `[WS] Initialize Cargo workspace and crate skeleton`
- `ETH2077-13` `[WS] Define cross-crate interfaces and trait boundaries`
- `ETH2077-14` `[WS] Add Lean proofs and integration-test scaffolding`

### Parent `ETH2077-3` (theorem registry)
- `ETH2077-15` `[THM] Create theorem ID taxonomy (Tier 1-3)`
- `ETH2077-16` `[THM] Build machine-readable theorem registry format`
- `ETH2077-17` `[THM] Add assumption mapping checker script`

### Parent `ETH2077-4` (CI formal gates)
- `ETH2077-18` `[CI] Add formal gate workflow for proof debt checks`
- `ETH2077-19` `[CI] Add strict release-branch no-placeholder enforcement`
- `ETH2077-20` `[CI] Upload proof and benchmark gate reports as artifacts`

### Parent `ETH2077-5` (OOB adapter)
- `ETH2077-21` `[OOB] Define adapter traits and commitment envelope types`
- `ETH2077-22` `[OOB] Implement mock OOB backend with deterministic behavior`
- `ETH2077-23` `[OOB] Add deterministic replay and anti-equivocation tests`

### Parent `ETH2077-6` (Engine API)
- `ETH2077-24` `[ENG] Scaffold Engine API server endpoints`
- `ETH2077-25` `[ENG] Implement execution bridge and payload conversion layer`
- `ETH2077-26` `[ENG] Add differential harness against reference clients`

### Parent `ETH2077-7` (benchmark harness)
- `ETH2077-27` `[BENCH] Implement deterministic workload generator`
- `ETH2077-28` `[BENCH] Build benchmark runner with TPS/finality metrics capture`
- `ETH2077-29` `[BENCH] Add benchmark artifact signing and publication pipeline`

### Parent `ETH2077-8` (module port + proof closure)
- `ETH2077-30` `[PORT] Select first Citadel critical module and migration plan`
- `ETH2077-31` `[PORT] Eliminate placeholders and pass full proof build for module`
- `ETH2077-32` `[PORT] Integrate migrated module into runtime with proof trace link`

### Parent `ETH2077-9` (witness/data-diff)
- `ETH2077-33` `[WIT] Define witness schema and CID commitment strategy`
- `ETH2077-34` `[WIT] Integrate SPORE diff-sync into witness propagation path`
- `ETH2077-35` `[WIT] Add transfer + integrity verification for witness plane`

### Parent `ETH2077-10` (adversarial/fault injection)
- `ETH2077-36` `[ADV] Build partition/churn simulator for network stress`
- `ETH2077-37` `[ADV] Add equivocation/replay attack scenario suite`
- `ETH2077-38` `[ADV] Build crash/recovery fault-injection suite`

### Parent `ETH2077-11` (threat model)
- `ETH2077-39` `[THREAT] Draft ETH2077 threat model v1`
- `ETH2077-40` `[THREAT] Create assumption ledger with ownership`
- `ETH2077-41` `[THREAT] Publish theorem-to-assumption matrix review gate`

## Pending Once Plane Reconnects

Move these to `In Progress` as Sprint 0:
- `ETH2077-2`
- `ETH2077-3`
- `ETH2077-4`
- `ETH2077-12`
- `ETH2077-18`

## Status Update (via Palace CLI)

Completed:
- `ETH2077-2` -> `In Progress`
- `ETH2077-3` -> `In Progress`
- `ETH2077-4` -> `In Progress`
- `ETH2077-12` -> `In Progress`
- `ETH2077-15` -> `In Progress`
- `ETH2077-18` -> `In Progress`

Note:
- Runtime/client implementation language is Rust.
- Lean remains the formal proof lane.
