# ETH2077 EIP Step Gate (eip-4844) - 2026-03-02

## Summary

- verdict: **PASS**
- scenario_set: `default`
- seed: `2077`
- tx_count per scenario: `200000`
- max sustained TPS: **654668**
- worst-case p99 finality (ms): **278.6**
- top-scenario bottleneck: **execution**

## Test Output (tail)

```
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.39s
     Running unittests src/bin/eth2077-devnetd.rs (target/debug/deps/eth2077_devnetd-4c4cd0afb3b45207)

running 2 tests
test tests::decode_unknown_typed_tx_reports_supported_types ... ok
test tests::decode_eip4844_raw_tx_extracts_blob_fields ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## Formal Gate Output

```
ETH2077 local formal gate
- proofs_dir: /mnt/riffcastle/castle/garage/ETH2077/proofs
- lean_files: 4
- sorry: 0
- axiom: 0
PASS: no placeholder debt detected
```

## Benchmark Output (tail)

```
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.36s
     Running `target/debug/eth2077-bench --scenario-set default --seed 2077 --tx-count 200000 --output-json /mnt/riffcastle/castle/garage/ETH2077/reports/eth2077-mesh-bench-2026-03-02-eip-4844.json --output-md /mnt/riffcastle/castle/garage/ETH2077/reports/eth2077-mesh-bench-2026-03-02-eip-4844.md`
Wrote JSON report: /mnt/riffcastle/castle/garage/ETH2077/reports/eth2077-mesh-bench-2026-03-02-eip-4844.json
Wrote Markdown report: /mnt/riffcastle/castle/garage/ETH2077/reports/eth2077-mesh-bench-2026-03-02-eip-4844.md
mesh-8n-baseline => TPS 271721, cap ingress/exec/oob 440000/303848/404686, finality p50/p95/p99 177.7/269.2/278.6 ms (bottleneck: execution)
mesh-16n-baseline => TPS 456407, cap ingress/exec/oob 880000/607118/762540, finality p50/p95/p99 155.4/200.7/208.9 ms (bottleneck: execution)
mesh-32n-baseline => TPS 624890, cap ingress/exec/oob 1760000/1213507/1418629, finality p50/p95/p99 173.3/199.7/202.3 ms (bottleneck: execution)
mesh-48n-scale => TPS 654668, cap ingress/exec/oob 2640000/1819805/2006211, finality p50/p95/p99 203.6/223.0/228.1 ms (bottleneck: execution)
mesh-32n-adversarial => TPS 531171, cap ingress/exec/oob 1760000/1208704/1361421, finality p50/p95/p99 223.7/252.6/259.5 ms (bottleneck: execution)
```

## Artifacts

- `/mnt/riffcastle/castle/garage/ETH2077/reports/eth2077-mesh-bench-2026-03-02-eip-4844.json`
- `/mnt/riffcastle/castle/garage/ETH2077/reports/eth2077-mesh-bench-2026-03-02-eip-4844.md`
- `/mnt/riffcastle/castle/garage/ETH2077/reports/eip-step-gate-2026-03-02-eip-4844.md`
