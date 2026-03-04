#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

RPC_URL_PRIMARY="${RPC_URL_PRIMARY:-http://127.0.0.1:9545}"
RPC_URLS="${RPC_URLS:-${RPC_URL_PRIMARY}}"
SENDER="${SENDER:-0x375249129507aec9309abc1f5c055494200c7c32}"
TO="${TO:-0x1111111111111111111111111111111111111111}"

TICK_MS_VALUES="${TICK_MS_VALUES:-500,750,1000}"
GAS_LIMIT_VALUES="${GAS_LIMIT_VALUES:-21000,30000}"
SENDER_COUNT_VALUES="${SENDER_COUNT_VALUES:-1,4,8}"
TX_COUNT="${TX_COUNT:-2000}"
POLL_MS="${POLL_MS:-25}"
DEADLINE_SECONDS="${DEADLINE_SECONDS:-180}"

DATE_TAG="$(date +%Y-%m-%d-%H%M%S)"
RESULTS_TSV="${ROOT_DIR}/reports/live-tps-sweep-${DATE_TAG}.tsv"
RESULTS_JSON="${ROOT_DIR}/reports/live-tps-sweep-${DATE_TAG}.json"
RESULTS_MD="${ROOT_DIR}/reports/live-tps-sweep-${DATE_TAG}.md"

IFS=',' read -r -a TICKS <<< "${TICK_MS_VALUES}"
IFS=',' read -r -a GAS_LIMITS <<< "${GAS_LIMIT_VALUES}"
IFS=',' read -r -a SENDER_COUNTS <<< "${SENDER_COUNT_VALUES}"

echo -e "tick_ms\tsender_count\tgas_limit\ttx_submitted\ttx_confirmed\tsubmit_tps\tconfirmed_tps\tp50_ms\tp95_ms\tp99_ms\treport_json" > "${RESULTS_TSV}"

cd "${ROOT_DIR}"
cargo build -p eth2077-live-bench --release >/dev/null

for tick in "${TICKS[@]}"; do
  echo "[sweep] tick_ms=${tick}: restarting devnet"
  bash "${ROOT_DIR}/scripts/devnet_down.sh" >/dev/null 2>&1 || true
  TICK_MS="${tick}" bash "${ROOT_DIR}/scripts/devnet_up.sh" >/dev/null

  if ! curl -fsS "${RPC_URL_PRIMARY}/healthz" >/dev/null 2>&1; then
    echo "error: primary RPC not healthy after devnet start: ${RPC_URL_PRIMARY}" >&2
    exit 1
  fi

  for sender_count in "${SENDER_COUNTS[@]}"; do
    workers="${sender_count}"
    for gas_limit in "${GAS_LIMITS[@]}"; do
      run_tag="tick${tick}-s${sender_count}-g${gas_limit}"
      out_json="${ROOT_DIR}/reports/live-tps-${DATE_TAG}-${run_tag}.json"
      out_md="${ROOT_DIR}/reports/live-tps-${DATE_TAG}-${run_tag}.md"

      echo "[sweep] run ${run_tag}"
      TICK_MS="${tick}" SENDER_COUNT="${sender_count}" GAS_LIMIT="${gas_limit}" TX_COUNT="${TX_COUNT}" RPC_URLS="${RPC_URLS}" \
      cargo run -p eth2077-live-bench --release -- \
        --rpc-urls "${RPC_URLS}" \
        --sender "${SENDER}" \
        --to "${TO}" \
        --sender-count "${sender_count}" \
        --workers "${workers}" \
        --tx-count "${TX_COUNT}" \
        --gas-limit "${gas_limit}" \
        --poll-ms "${POLL_MS}" \
        --deadline-seconds "${DEADLINE_SECONDS}" \
        --output-json "${out_json}" \
        --output-md "${out_md}" >/dev/null

      TICK_MS="${tick}" SENDER_COUNT="${sender_count}" GAS_LIMIT="${gas_limit}" TX_COUNT="${TX_COUNT}" RPC_URLS="${RPC_URLS}" \
      bash "${ROOT_DIR}/scripts/sign_benchmark_artifact.sh" "${out_json}" "${out_md}" >/dev/null

      metrics_tsv="$(jq -r '[.tx_submitted,.tx_confirmed,.submit_tps,.confirmed_tps,.p50_confirmation_ms,.p95_confirmation_ms,.p99_confirmation_ms] | @tsv' "${out_json}")"
      echo -e "${tick}\t${sender_count}\t${gas_limit}\t${metrics_tsv}\t${out_json}" >> "${RESULTS_TSV}"
    done
  done
done

jq -R -s '
  split("\n")
  | map(select(length > 0))
  | .[1:]
  | map(split("\t"))
  | map({
      tick_ms: (.[0] | tonumber),
      sender_count: (.[1] | tonumber),
      gas_limit: (.[2] | tonumber),
      tx_submitted: (.[3] | tonumber),
      tx_confirmed: (.[4] | tonumber),
      submit_tps: (.[5] | tonumber),
      confirmed_tps: (.[6] | tonumber),
      p50_ms: (.[7] | tonumber),
      p95_ms: (.[8] | tonumber),
      p99_ms: (.[9] | tonumber),
      report_json: .[10]
    })
' "${RESULTS_TSV}" > "${RESULTS_JSON}.runs"

jq -n \
  --arg created_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  --arg rpc_urls "${RPC_URLS}" \
  --arg tick_values "${TICK_MS_VALUES}" \
  --arg gas_values "${GAS_LIMIT_VALUES}" \
  --arg sender_values "${SENDER_COUNT_VALUES}" \
  --arg tx_count "${TX_COUNT}" \
  --slurpfile runs "${RESULTS_JSON}.runs" \
  '{
    kind: "eth2077-live-tps-sweep-v1",
    created_at_utc: $created_at,
    matrix: {
      rpc_urls: $rpc_urls,
      tick_ms_values: $tick_values,
      gas_limit_values: $gas_values,
      sender_count_values: $sender_values,
      tx_count: ($tx_count | tonumber)
    },
    runs: $runs[0],
    best_overall: ($runs[0] | max_by(.confirmed_tps)),
    best_by_tick: ($runs[0] | group_by(.tick_ms) | map(max_by(.confirmed_tps)))
  }' > "${RESULTS_JSON}"
rm -f "${RESULTS_JSON}.runs"

{
  echo "# ETH2077 Live TPS Sweep"
  echo
  echo "- created_at: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "- rpc_urls: ${RPC_URLS}"
  echo "- tick_ms_values: ${TICK_MS_VALUES}"
  echo "- gas_limit_values: ${GAS_LIMIT_VALUES}"
  echo "- sender_count_values: ${SENDER_COUNT_VALUES}"
  echo "- tx_count per run: ${TX_COUNT}"
  echo
  echo "## Best Overall"
  echo
  jq -r '"- tick_ms=\(.best_overall.tick_ms), sender_count=\(.best_overall.sender_count), gas_limit=\(.best_overall.gas_limit), confirmed_tps=\(.best_overall.confirmed_tps|floor), p99=\(.best_overall.p99_ms|floor)ms"' "${RESULTS_JSON}"
  echo
  echo "## Ranked Runs"
  echo
  echo "| Tick (ms) | Sender Count | Gas Limit | Confirmed TPS | Submit TPS | p50 (ms) | p95 (ms) | p99 (ms) | Report |"
  echo "|---:|---:|---:|---:|---:|---:|---:|---:|---|"
  tail -n +2 "${RESULTS_TSV}" | sort -t$'\t' -k7,7nr | while IFS=$'\t' read -r tick sender_count gas tx_submitted tx_confirmed submit_tps confirmed_tps p50 p95 p99 report_json; do
    printf '| %s | %s | %s | %.0f | %.0f | %.1f | %.1f | %.1f | `%s` |\n' \
      "$tick" "$sender_count" "$gas" "$confirmed_tps" "$submit_tps" "$p50" "$p95" "$p99" "$report_json"
  done
} > "${RESULTS_MD}"

echo "Wrote sweep artifacts:"
echo "- ${RESULTS_TSV}"
echo "- ${RESULTS_JSON}"
echo "- ${RESULTS_MD}"
