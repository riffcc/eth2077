#!/usr/bin/env bash
set -euo pipefail

if ! command -v curl >/dev/null 2>&1; then
  echo "error: curl is required" >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

DEVNET_DIR="${DEVNET_DIR:-${ROOT_DIR}/artifacts/devnet-local}"
NODE_COUNT="${NODE_COUNT:-4}"
NETWORK="${NETWORK:-eth2077-devnet-local}"
SEED="${SEED:-2077}"
CHAIN_ID="${CHAIN_ID:-2077002}"
GENESIS_TS="${GENESIS_TS:-1772409600}"
FORK_EPOCH="${FORK_EPOCH:-0}"
ALLOC_ACCOUNTS="${ALLOC_ACCOUNTS:-128}"
RPC_BASE_PORT="${RPC_BASE_PORT:-9545}"
P2P_BASE_PORT="${P2P_BASE_PORT:-30303}"
TICK_MS="${TICK_MS:-1000}"
LOG_DIR="${DEVNET_DIR}/logs"
PID_FILE="${DEVNET_DIR}/pids.tsv"
NODES_FILE="${DEVNET_DIR}/nodes.json"

if [[ "${NODE_COUNT}" -lt 1 ]]; then
  echo "error: NODE_COUNT must be >= 1" >&2
  exit 1
fi

if [[ "${NODE_COUNT}" -lt 3 ]]; then
  BOOTNODES="${BOOTNODES:-${NODE_COUNT}}"
else
  BOOTNODES="${BOOTNODES:-3}"
fi

mkdir -p "${DEVNET_DIR}" "${LOG_DIR}"

if [[ -f "${PID_FILE}" ]]; then
  echo "Existing PID file found; stopping previous devnet first."
  bash "${ROOT_DIR}/scripts/devnet_down.sh" || true
fi

echo "Building deterministic devnet artifacts"
OUTPUT_DIR="${DEVNET_DIR}" \
NETWORK="${NETWORK}" \
SEED="${SEED}" \
CHAIN_ID="${CHAIN_ID}" \
GENESIS_TS="${GENESIS_TS}" \
FORK_EPOCH="${FORK_EPOCH}" \
VALIDATORS="${NODE_COUNT}" \
BOOTNODES="${BOOTNODES}" \
ALLOC_ACCOUNTS="${ALLOC_ACCOUNTS}" \
  bash "${ROOT_DIR}/scripts/build_testnet_artifacts.sh"

echo "Building eth2077-devnet binary"
cd "${ROOT_DIR}"
cargo build -p eth2077-node --bin eth2077-devnet

: > "${PID_FILE}"

for ((i=0; i<NODE_COUNT; i++)); do
  rpc_port=$((RPC_BASE_PORT + i))
  p2p_port=$((P2P_BASE_PORT + i))
  node_dir="${DEVNET_DIR}/node-${i}"
  log_file="${LOG_DIR}/node-${i}.log"
  mkdir -p "${node_dir}"

  nohup "${ROOT_DIR}/target/debug/eth2077-devnet" \
    --node-id "${i}" \
    --nodes "${NODE_COUNT}" \
    --rpc-port "${rpc_port}" \
    --p2p-port "${p2p_port}" \
    --tick-ms "${TICK_MS}" \
    --chain-spec "${DEVNET_DIR}/chain-spec.json" \
    --data-dir "${node_dir}" > "${log_file}" 2>&1 &

  pid="$!"
  echo "${pid}	${i}	${rpc_port}	${p2p_port}	${log_file}" >> "${PID_FILE}"
done

sleep 1

echo "Running health checks"
for ((i=0; i<NODE_COUNT; i++)); do
  rpc_port=$((RPC_BASE_PORT + i))
  if ! curl -fsS "http://127.0.0.1:${rpc_port}/healthz" >/dev/null; then
    echo "error: node ${i} failed health check on port ${rpc_port}" >&2
    bash "${ROOT_DIR}/scripts/devnet_down.sh" || true
    exit 1
  fi
done

{
  echo "["
  for ((i=0; i<NODE_COUNT; i++)); do
    rpc_port=$((RPC_BASE_PORT + i))
    p2p_port=$((P2P_BASE_PORT + i))
    comma=","
    if [[ "${i}" -eq $((NODE_COUNT - 1)) ]]; then
      comma=""
    fi
    printf '  {"node_id": %d, "rpc": "http://127.0.0.1:%d", "p2p_port": %d}%s\n' \
      "${i}" "${rpc_port}" "${p2p_port}" "${comma}"
  done
  echo "]"
} > "${NODES_FILE}"

echo "Devnet deployed"
echo "- node_count: ${NODE_COUNT}"
echo "- network: ${NETWORK}"
echo "- chain_spec: ${DEVNET_DIR}/chain-spec.json"
echo "- pid_file: ${PID_FILE}"
echo "- nodes_file: ${NODES_FILE}"
echo "Use: bash scripts/devnet_status.sh"
echo "Use: bash scripts/devnet_down.sh"
