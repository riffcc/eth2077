# ETH2077 YOLO Roadmap (Execution Mode)

Date: 2026-03-02

This is the high-velocity execution profile for ETH2077 when the goal is to ship evidence fast, not discuss theory.

## Objective

1. Keep ETH2077 demonstrably live and usable.
2. Increase reproducible throughput and tighten finality.
3. Advance formal gates in lockstep with performance claims.

## Non-Negotiables

1. No benchmark artifact means no public claim.
2. No "formally verified" language beyond gate status.
3. No synthetic shortcuts in default user-facing paths.

## Daily Strike Loop

1. Run YOLO runner:
   - `bash scripts/yolo_roadmap_now.sh`
2. Read the generated report in `reports/`.
3. Pick bottleneck lane from report:
   - `execution`
   - `oob_consensus`
   - `ingress`
4. Ship one measurable optimization in the bottleneck lane.
5. Re-run and compare numbers before/after.

## This Week's Priority Stack

1. Execution lane optimization:
   - Improve tx execution throughput and commit path.
   - Preserve deterministic replay and receipt correctness.
2. OOB lane optimization:
   - Reduce finality lag under byzantine and packet-loss stress.
3. Reliability lane:
   - Keep explorer/wallet/market/observatory paths continuously demoable.
4. Formal lane:
   - Burn down placeholder debt on critical modules.
   - Keep theorem acceptance tied to CI and reports.

## Definition Of "Busy ETH2077"

1. Public demo surfaces are live and coherent (`wallet`, `market`, `explorer`, `2077`).
2. Deterministic benchmark report is fresh (same day).
3. Formal gate report is fresh and attached to sprint artifacts.
4. At least one bottleneck-focused optimization lands per cycle.

## Plane Sync

Use Plane project `ETH2077` as source of truth and keep a top-level execution issue tracking:

- latest benchmark report path,
- latest formal gate status,
- current bottleneck and mitigation,
- next optimization experiment.

