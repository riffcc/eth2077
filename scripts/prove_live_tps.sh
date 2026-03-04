#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

RPC_URL="${RPC_URL:-http://127.0.0.1:9545}"
RPC_URLS="${RPC_URLS:-${RPC_URL}}"
SENDER="${SENDER:-0x375249129507aec9309abc1f5c055494200c7c32}"
TO="${TO:-0x1111111111111111111111111111111111111111}"
SENDER_COUNT="${SENDER_COUNT:-4}"
WORKERS="${WORKERS:-4}"
TX_COUNT="${TX_COUNT:-2000}"
GAS_LIMIT="${GAS_LIMIT:-21000}"
POLL_MS="${POLL_MS:-25}"
DEADLINE_SECONDS="${DEADLINE_SECONDS:-120}"

DATE_TAG="$(date +%Y-%m-%d-%H%M%S)"
OUT_JSON="${ROOT_DIR}/reports/live-tps-${DATE_TAG}.json"
OUT_MD="${ROOT_DIR}/reports/live-tps-${DATE_TAG}.md"

echo "Checking local devnet"
if ! curl -fsS "${RPC_URL}/healthz" >/dev/null 2>&1; then
  echo "No active devnet detected. Starting one now."
  bash "${ROOT_DIR}/scripts/devnet_up.sh"
fi

echo "Running live TPS benchmark"
cd "${ROOT_DIR}"
cargo run -p eth2077-live-bench --release -- \
  --rpc-urls "${RPC_URLS}" \
  --sender "${SENDER}" \
  --to "${TO}" \
  --sender-count "${SENDER_COUNT}" \
  --workers "${WORKERS}" \
  --tx-count "${TX_COUNT}" \
  --gas-limit "${GAS_LIMIT}" \
  --poll-ms "${POLL_MS}" \
  --deadline-seconds "${DEADLINE_SECONDS}" \
  --output-json "${OUT_JSON}" \
  --output-md "${OUT_MD}"

bash "${ROOT_DIR}/scripts/sign_benchmark_artifact.sh" "${OUT_JSON}" "${OUT_MD}"

echo "Wrote live benchmark artifacts:"
echo "- ${OUT_JSON}"
echo "- ${OUT_MD}"
