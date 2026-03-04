#!/usr/bin/env bash
set -euo pipefail

NETWORK="${NETWORK:-eth2077-devnet-docker}"
SEED="${SEED:-2077}"
CHAIN_ID="${CHAIN_ID:-2077003}"
GENESIS_TS="${GENESIS_TS:-1772409600}"
FORK_EPOCH="${FORK_EPOCH:-0}"
VALIDATORS="${VALIDATORS:-4}"
BOOTNODES="${BOOTNODES:-3}"
ALLOC_ACCOUNTS="${ALLOC_ACCOUNTS:-128}"
NODE_COUNT="${NODE_COUNT:-4}"
RPC_PORT="${RPC_PORT:-8545}"
P2P_PORT="${P2P_PORT:-30303}"
TICK_MS="${TICK_MS:-1000}"

mkdir -p /data

eth2077-testnet \
  --output-dir /data \
  --network "${NETWORK}" \
  --seed "${SEED}" \
  --chain-id "${CHAIN_ID}" \
  --genesis-ts "${GENESIS_TS}" \
  --fork-epoch "${FORK_EPOCH}" \
  --validators "${VALIDATORS}" \
  --bootnodes "${BOOTNODES}" \
  --alloc-accounts "${ALLOC_ACCOUNTS}"

exec eth2077-devnetd \
  --rpc-host 0.0.0.0 \
  --node-id 0 \
  --nodes "${NODE_COUNT}" \
  --rpc-port "${RPC_PORT}" \
  --p2p-port "${P2P_PORT}" \
  --tick-ms "${TICK_MS}" \
  --chain-spec /data/chain-spec.json \
  --data-dir /data/node-0
