#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

OUTPUT_DIR="${OUTPUT_DIR:-${ROOT_DIR}/artifacts/testnet-alpha}"
NETWORK="${NETWORK:-eth2077-alpha}"
SEED="${SEED:-2077}"
CHAIN_ID="${CHAIN_ID:-2077001}"
GENESIS_TS="${GENESIS_TS:-1772409600}"
FORK_EPOCH="${FORK_EPOCH:-0}"
VALIDATORS="${VALIDATORS:-48}"
BOOTNODES="${BOOTNODES:-8}"
ALLOC_ACCOUNTS="${ALLOC_ACCOUNTS:-512}"

echo "Building deterministic ETH2077 testnet artifacts"
echo "- output_dir: ${OUTPUT_DIR}"
echo "- network: ${NETWORK}"
echo "- seed: ${SEED}"
echo "- chain_id: ${CHAIN_ID}"
echo "- validators: ${VALIDATORS}"
echo "- bootnodes: ${BOOTNODES}"

cd "${ROOT_DIR}"
cargo run -p eth2077-testnet --release -- \
  --output-dir "${OUTPUT_DIR}" \
  --network "${NETWORK}" \
  --seed "${SEED}" \
  --chain-id "${CHAIN_ID}" \
  --genesis-ts "${GENESIS_TS}" \
  --fork-epoch "${FORK_EPOCH}" \
  --validators "${VALIDATORS}" \
  --bootnodes "${BOOTNODES}" \
  --alloc-accounts "${ALLOC_ACCOUNTS}"

(
  cd "${OUTPUT_DIR}"
  sha256sum chain-spec.json genesis.json metadata.json bootnodes.txt > SHA256SUMS
)

echo "Wrote checksums: ${OUTPUT_DIR}/SHA256SUMS"
