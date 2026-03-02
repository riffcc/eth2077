# ETH2077 Deterministic Mesh Benchmark

This is a deterministic synthetic benchmark for ETH-like workload flow over a Citadel-mesh-style model.
It is a planning baseline, not yet a live full-client throughput claim.

## Parameters

- scenario_set: `bottleneck-48n`
- seed: `2077`
- tx_count per scenario: `600000`
- commit_batch_size: `1024`

## Results

| Scenario | Nodes | Sustained TPS | Ingress Cap (TPS) | Exec Cap (TPS) | OOB Cap (TPS) | p50 Finality (ms) | p95 Finality (ms) | p99 Finality (ms) | Bottleneck |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---|
| mesh-48n-baseline-ing1.00-exec1.00-oob1.00 | 48 | 1140934 | 2640000 | 1819805 | 2006211 | 235.2 | 282.8 | 291.5 | execution |
| mesh-48n-exec-lift-ing1.00-exec1.15-oob1.00 | 48 | 1220472 | 2640000 | 2092776 | 2006211 | 221.0 | 254.9 | 262.3 | oob_consensus |
| mesh-48n-exec-lift-ing1.00-exec1.30-oob1.00 | 48 | 1216206 | 2640000 | 2365746 | 2006211 | 221.3 | 256.3 | 263.9 | oob_consensus |
| mesh-48n-exec-lift-ing1.00-exec1.50-oob1.00 | 48 | 1218551 | 2640000 | 2729707 | 2006211 | 221.4 | 254.3 | 262.1 | oob_consensus |
| mesh-48n-exec-lift-ing1.00-exec1.80-oob1.00 | 48 | 1215414 | 2640000 | 3275649 | 2006211 | 221.5 | 255.5 | 262.6 | oob_consensus |
| mesh-48n-exec-lift-ing1.00-exec2.20-oob1.00 | 48 | 1217601 | 2640000 | 4003571 | 2006211 | 221.0 | 256.0 | 263.9 | oob_consensus |
| mesh-48n-exec-oob-lift-ing1.00-exec1.80-oob1.30 | 48 | 1407035 | 2640000 | 3275649 | 2608074 | 186.2 | 196.9 | 198.0 | oob_consensus |
| mesh-48n-exec-oob-lift-ing1.00-exec2.20-oob1.60 | 48 | 1415636 | 2640000 | 4003571 | 3209937 | 185.5 | 195.3 | 196.2 | ingress |
| mesh-48n-ingress-exec-oob-lift-ing1.30-exec2.20-oob2.00 | 48 | 1616171 | 3432000 | 4003571 | 4012422 | 185.5 | 195.1 | 196.1 | ingress |

## Notes

1. Numbers are deterministic under fixed seed and scenario definitions.
2. Next step is wiring the same harness shape to live node runtime paths for empirical multi-node validation.
