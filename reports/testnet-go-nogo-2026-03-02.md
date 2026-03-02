# ETH2077 Testnet Go/No-Go (2026-03-02)

## Summary

- verdict: **PASS**
- max sustained TPS (default deterministic suite): **1144481**
- worst-case p99 finality (ms): **679.4**
- artifacts dir: `/mnt/riffcastle/castle/garage/ETH2077/artifacts/testnet-alpha`

## Formal Gate Output

```
ETH2077 local formal gate
- proofs_dir: /mnt/riffcastle/castle/garage/ETH2077/proofs
- lean_files: 4
- sorry: 0
- axiom: 0
PASS: no placeholder debt detected
```

## Benchmark Command Output (tail)

```
    Finished `release` profile [optimized] target(s) in 0.07s
     Running `target/release/eth2077-bench --scenario-set default --seed 2077 --tx-count 600000 --output-json /mnt/riffcastle/castle/garage/ETH2077/reports/testnet-go-nogo-bench-2026-03-02.json --output-md /mnt/riffcastle/castle/garage/ETH2077/reports/testnet-go-nogo-bench-2026-03-02.md`
Wrote JSON report: /mnt/riffcastle/castle/garage/ETH2077/reports/testnet-go-nogo-bench-2026-03-02.json
Wrote Markdown report: /mnt/riffcastle/castle/garage/ETH2077/reports/testnet-go-nogo-bench-2026-03-02.md
mesh-8n-baseline => TPS 292150, cap ingress/exec/oob 440000/303848/404686, finality p50/p95/p99 380.9/655.6/679.4 ms (bottleneck: execution)
mesh-16n-baseline => TPS 546559, cap ingress/exec/oob 880000/607118/762540, finality p50/p95/p99 256.8/396.9/408.0 ms (bottleneck: execution)
mesh-32n-baseline => TPS 921912, cap ingress/exec/oob 1760000/1213507/1418629, finality p50/p95/p99 224.9/294.6/302.5 ms (bottleneck: execution)
mesh-48n-scale => TPS 1144481, cap ingress/exec/oob 2640000/1819805/2006211, finality p50/p95/p99 238.6/283.6/291.2 ms (bottleneck: execution)
mesh-32n-adversarial => TPS 850843, cap ingress/exec/oob 1760000/1208704/1361421, finality p50/p95/p99 278.6/346.8/358.1 ms (bottleneck: execution)
```

## Artifact Builder Output (tail)

```
Building deterministic ETH2077 testnet artifacts
- output_dir: /mnt/riffcastle/castle/garage/ETH2077/artifacts/testnet-alpha
- network: eth2077-alpha
- seed: 2077
- chain_id: 2077001
- validators: 48
- bootnodes: 8
    Finished `release` profile [optimized] target(s) in 0.05s
     Running `target/release/eth2077-testnet --output-dir /mnt/riffcastle/castle/garage/ETH2077/artifacts/testnet-alpha --network eth2077-alpha --seed 2077 --chain-id 2077001 --genesis-ts 1772409600 --fork-epoch 0 --validators 48 --bootnodes 8 --alloc-accounts 512`
Wrote deterministic testnet artifacts to: /mnt/riffcastle/castle/garage/ETH2077/artifacts/testnet-alpha
- chain-spec: /mnt/riffcastle/castle/garage/ETH2077/artifacts/testnet-alpha/chain-spec.json
- genesis: /mnt/riffcastle/castle/garage/ETH2077/artifacts/testnet-alpha/genesis.json
- metadata: /mnt/riffcastle/castle/garage/ETH2077/artifacts/testnet-alpha/metadata.json
- bootnodes: /mnt/riffcastle/castle/garage/ETH2077/artifacts/testnet-alpha/bootnodes.txt
Parameters: seed=2077, chain_id=2077001, validators=48, quorum=31, fault_budget=15
Wrote checksums: /mnt/riffcastle/castle/garage/ETH2077/artifacts/testnet-alpha/SHA256SUMS
```

## Required Files

- `/mnt/riffcastle/castle/garage/ETH2077/reports/testnet-go-nogo-bench-2026-03-02.json`
- `/mnt/riffcastle/castle/garage/ETH2077/reports/testnet-go-nogo-bench-2026-03-02.md`
- `/mnt/riffcastle/castle/garage/ETH2077/artifacts/testnet-alpha/chain-spec.json`
- `/mnt/riffcastle/castle/garage/ETH2077/artifacts/testnet-alpha/genesis.json`
- `/mnt/riffcastle/castle/garage/ETH2077/artifacts/testnet-alpha/metadata.json`
- `/mnt/riffcastle/castle/garage/ETH2077/artifacts/testnet-alpha/bootnodes.txt`
- `/mnt/riffcastle/castle/garage/ETH2077/artifacts/testnet-alpha/SHA256SUMS`
