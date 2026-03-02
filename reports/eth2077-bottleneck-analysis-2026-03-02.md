# ETH2077 Bottleneck Lift Analysis (2026-03-02)

## Objective

Identify the active execution bottleneck in the deterministic 48-node Citadel-mesh ETH model, quantify lift response, and formalize bottleneck-shift behavior in Lean.

## Configuration

- Repository: `/mnt/riffcastle/castle/garage/ETH2077`
- Benchmark harness: `crates/eth2077-bench`
- Scenario set: `bottleneck-48n`
- Seed: `2077`
- Transactions per scenario: `600000`
- Commit batch size: `1024`

## Command Evidence

```bash
cargo run -p eth2077-bench --release -- \
  --scenario-set bottleneck-48n \
  --seed 2077 \
  --tx-count 600000 \
  --output-json reports/eth2077-bottleneck-sweep-2026-03-02.json \
  --output-md reports/eth2077-bottleneck-sweep-2026-03-02.md

bash ./scripts/check_eth2077_formal_gates.sh --require-proofs
cd proofs && lake build ETH2077Proofs
```

## Performance Results

From [`eth2077-bottleneck-sweep-2026-03-02.md`](./eth2077-bottleneck-sweep-2026-03-02.md):

1. Baseline (`ing1.00 exec1.00 oob1.00`): **1,140,934 TPS**, bottleneck = `execution`.
2. Execution lift alone (`exec1.15`): **1,220,472 TPS** (+6.97%), bottleneck flips to `oob_consensus`.
3. Further execution-only lift (`exec1.30` to `exec2.20`): plateaus around **1.216M to 1.220M TPS**.
4. Execution + OOB lift (`exec1.80 oob1.30`): **1,407,035 TPS**, bottleneck still `oob_consensus`.
5. Larger execution + OOB lift (`exec2.20 oob1.60`): **1,415,636 TPS**, bottleneck flips to `ingress`.
6. Ingress + execution + OOB lift (`ing1.30 exec2.20 oob2.00`): **1,616,171 TPS** (+41.65% over baseline), bottleneck = `ingress`.

## Formal Proof Status

Lean module: `proofs/ETH2077Proofs/ExecutionOptimization.lean`

Key theorems now machine-checked:

- `sustained_scales_with_execution_until_next_bottleneck`
- `sustained_strict_gain_with_execution_factor`
- `non_execution_bottleneck_characterization`
- `lifting_execution_past_bottleneck_plateaus`

Gate status:

- Lean files: `3`
- `sorry`: `0`
- `axiom`: `0`
- Formal gate verdict: **PASS**

## Gate Verdict

- Bottleneck-identification gate: **PASS**
- Execution-lift formalization gate: **PASS**
- Multi-million deterministic model gate (>= 1.5M TPS): **PASS**

## Residual Risks

1. Results are deterministic synthetic model outputs, not yet live multi-node runtime measurements.
2. OOB and ingress modeling are coarse-grained; transport contention and crypto costs are abstracted.
3. Proofs validate the throughput model properties, not full EVM execution semantics.

## Next Moves

1. Bind this harness to real runtime paths (execution queue, signature verification, networking).
2. Add adversarial sweep at lifted ingress/oob to test p99 finality under stress.
3. Add Lean obligations for safety/liveness around out-of-band consensus decisions.
