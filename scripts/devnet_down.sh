#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
DEVNET_DIR="${DEVNET_DIR:-${ROOT_DIR}/artifacts/devnet-local}"
PID_FILE="${DEVNET_DIR}/pids.tsv"

if [[ ! -f "${PID_FILE}" ]]; then
  echo "No PID file found at ${PID_FILE}. Nothing to stop."
  exit 0
fi

echo "Stopping ETH2077 devnet"
while IFS=$'\t' read -r pid node_id _rpc _p2p _log; do
  if kill -0 "${pid}" >/dev/null 2>&1; then
    kill "${pid}" >/dev/null 2>&1 || true
    echo "- stopped node ${node_id} (pid ${pid})"
  else
    echo "- node ${node_id} already stopped (pid ${pid})"
  fi
done < "${PID_FILE}"

rm -f "${PID_FILE}"
echo "Devnet stopped"
