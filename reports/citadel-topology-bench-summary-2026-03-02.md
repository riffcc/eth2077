# Citadel Topology Benchmark Summary

Date: 2026-03-02

## Run Command

```bash
cd /mnt/riffcastle/lagun-project/citadel
cargo bench -p citadel-topology --bench topology_bench -- --sample-size 10
```

Raw log:
- `reports/citadel-topology-bench-2026-03-02.log`

## Environment Fix Applied Before Run

Removed misplaced non-root profile block from:
- `/mnt/riffcastle/lagun-project/citadel/crates/citadel-wasm/Cargo.toml`

Result:
- prior Cargo profile warning is gone in this run.

## Key Metrics

- `spiral_to_coord/0`: `1.4702 ns .. 1.4813 ns` (`675.07 .. 680.17 Melem/s`)
- `spiral_to_coord/1000`: `37.077 ns .. 37.429 ns`
- `spiral_to_coord/1000000`: `39.051 ns .. 39.461 ns`
- `coord_to_spiral/shell/100`: `80.051 ns .. 83.282 ns`
- `neighbors/shell/0`: `21.659 ns .. 21.733 ns`
- `roundtrip/0`: `3.0954 ns .. 3.2692 ns`
- `routing_scale/shell_distance/5`: `362.71 ns .. 364.25 ns`
- `routing_scale/shell_distance/10`: `720.75 ns .. 728.38 ns`
- `routing_scale/shell_distance/20`: `2.3542 us .. 2.4505 us`
- `routing_scale/shell_distance/50`: `5.3287 us .. 5.7218 us`

## Interpretation

This is a mesh/topology microbenchmark baseline, not an end-to-end Ethereum TPS/finality result.

To get ETH-network numbers, next benchmarks must include:
1. transaction ingestion workload,
2. out-of-band commitment path,
3. execution bridge path,
4. multi-node network conditions.
