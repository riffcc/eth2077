# ETH2077 Deterministic Mesh Benchmark

This is a deterministic synthetic benchmark for ETH-like workload flow over a Citadel-mesh-style model.
It is a planning baseline, not yet a live full-client throughput claim.

## Parameters

- scenario_set: `default`
- seed: `2077`
- tx_count per scenario: `600000`
- commit_batch_size: `1024`

## Results

| Scenario | Nodes | Sustained TPS | Ingress Cap (TPS) | Exec Cap (TPS) | OOB Cap (TPS) | p50 Finality (ms) | p95 Finality (ms) | p99 Finality (ms) | Bottleneck |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---|
| mesh-8n-baseline | 8 | 292150 | 440000 | 303848 | 404686 | 380.9 | 655.6 | 679.4 | execution |
| mesh-16n-baseline | 16 | 546559 | 880000 | 607118 | 762540 | 256.8 | 396.9 | 408.0 | execution |
| mesh-32n-baseline | 32 | 921912 | 1760000 | 1213507 | 1418629 | 224.9 | 294.6 | 302.5 | execution |
| mesh-48n-scale | 48 | 1144481 | 2640000 | 1819805 | 2006211 | 238.6 | 283.6 | 291.2 | execution |
| mesh-32n-adversarial | 32 | 850843 | 1760000 | 1208704 | 1361421 | 278.6 | 346.8 | 358.1 | execution |

## Notes

1. Numbers are deterministic under fixed seed and scenario definitions.
2. Next step is wiring the same harness shape to live node runtime paths for empirical multi-node validation.
