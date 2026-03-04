# ETH2077 Live TPS Proof

`eth2077-bench` is deterministic and synthetic.  
Use this flow when we need **actual measured TPS** from live JSON-RPC submission and receipt confirmation.
Default mode targets a single RPC endpoint (single-lane chain measurement).

## One-command Run

```bash
bash scripts/prove_live_tps.sh
```

Optional environment overrides:

```bash
RPC_URL=http://127.0.0.1:9545 \
TX_COUNT=4000 \
POLL_MS=20 \
DEADLINE_SECONDS=180 \
bash scripts/prove_live_tps.sh
```

Multi-sender load example:

```bash
RPC_URLS=http://127.0.0.1:9545,http://127.0.0.1:9546,http://127.0.0.1:9547,http://127.0.0.1:9548 \
SENDER_COUNT=8 \
WORKERS=8 \
GAS_LIMIT=21000 \
TX_COUNT=4000 \
bash scripts/prove_live_tps.sh
```

The multi-endpoint example above is an aggregate lane stress test; check `chain_txs_in_spanned_blocks` and notes to avoid over-claiming single-chain TPS.

## Artifacts

The run writes:

- `reports/live-tps-<timestamp>.json`
- `reports/live-tps-<timestamp>.md`

Key fields:

- `submit_tps`: RPC submission throughput.
- `confirmed_tps`: confirmed transactions per second over the confirmation window.
- `p50/p95/p99_confirmation_ms`: end-to-end send->receipt latency percentiles.
- `chain_txs_in_spanned_blocks`: chain-level tx count over benchmark block span.

## Current Caveat

Current devnet node requires sender hint priming via `eth_getTransactionCount(sender)` before each `eth_sendRawTransaction`.
The live benchmark accounts for this and records `sender_hint_mode: true` in the report.

## Sweep Mode

Run a matrix sweep across `tick_ms`, `gas_limit`, and `sender_count`:

```bash
TICK_MS_VALUES=500,750,1000 \
GAS_LIMIT_VALUES=21000,30000 \
SENDER_COUNT_VALUES=1,4,8 \
TX_COUNT=2000 \
bash scripts/prove_live_tps_sweep.sh
```

Sweep outputs:

- `reports/live-tps-sweep-<timestamp>.tsv`
- `reports/live-tps-sweep-<timestamp>.json`
- `reports/live-tps-sweep-<timestamp>.md`

## Signed Evidence Bundle

Every run now writes:

- `<report>.sha256`
- `<report>.manifest.json`

Optional identity signature:

```bash
SIGNING_KEY_PATH=/path/to/private.pem bash scripts/prove_live_tps.sh
```

If `SIGNING_KEY_PATH` is set, the flow also writes:

- `<report>.manifest.sig`
- `<report>.manifest.pub.pem`
