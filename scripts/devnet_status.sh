#!/usr/bin/env bash
set -euo pipefail

if ! command -v curl >/dev/null 2>&1; then
  echo "error: curl is required" >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
DEVNET_DIR="${DEVNET_DIR:-${ROOT_DIR}/artifacts/devnet-local}"
PID_FILE="${DEVNET_DIR}/pids.tsv"

if [[ ! -f "${PID_FILE}" ]]; then
  echo "No devnet PID file found at ${PID_FILE}"
  exit 1
fi

printf "%-6s %-8s %-8s %-12s %-14s %-14s %-14s\n" "node" "pid" "alive" "height" "finalized" "chain_id" "network"

while IFS=$'\t' read -r pid node_id rpc_port _p2p _log; do
  alive="no"
  if kill -0 "${pid}" >/dev/null 2>&1; then
    alive="yes"
  fi

  if status_json="$(curl -fsS "http://127.0.0.1:${rpc_port}/status" 2>/dev/null)"; then
    height="$(jq -r '.current_height' <<<"${status_json}")"
    finalized="$(jq -r '.finalized_height' <<<"${status_json}")"
    chain_id="$(jq -r '.chain_id' <<<"${status_json}")"
    network="$(jq -r '.network' <<<"${status_json}")"
  else
    height="-"
    finalized="-"
    chain_id="-"
    network="-"
  fi

  printf "%-6s %-8s %-8s %-12s %-14s %-14s %-14s\n" \
    "${node_id}" "${pid}" "${alive}" "${height}" "${finalized}" "${chain_id}" "${network}"
done < "${PID_FILE}"
