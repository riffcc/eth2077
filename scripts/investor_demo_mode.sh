#!/usr/bin/env bash
set -euo pipefail

if ! command -v curl >/dev/null 2>&1; then
  echo "error: curl is required" >&2
  exit 1
fi

if ! command -v jq >/dev/null 2>&1; then
  echo "error: jq is required" >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

EXPLORER_HOST="${EXPLORER_HOST:-explorer.riff.cc}"
WALLET_HOST="${WALLET_HOST:-wallet.riff.cc}"
MARKET_HOST="${MARKET_HOST:-market.riff.cc}"
OBSERVATORY_HOST="${OBSERVATORY_HOST:-2077.riff.cc}"
RPC_HOST="${RPC_HOST:-rpc.riff.cc}"
EDGE_IP="${EDGE_IP:-}"

EXPLORER_URL="https://${EXPLORER_HOST}"
WALLET_URL="https://${WALLET_HOST}"
MARKET_URL="https://${MARKET_HOST}"
OBSERVATORY_URL="https://${OBSERVATORY_HOST}"
RPC_URL="https://${RPC_HOST}"

EXPECTED_CHAIN_ID_HEX="${EXPECTED_CHAIN_ID_HEX:-0x1fb14b}"
START_STACK="${START_STACK:-0}"
WAIT_TRIES="${WAIT_TRIES:-60}"
WAIT_DELAY_SECONDS="${WAIT_DELAY_SECONDS:-2}"

DEMO_ADDRESS="${DEMO_ADDRESS:-}"
DEMO_BALANCE_WEI="${DEMO_BALANCE_WEI:-0x21e19e0c9bab2400000}" # 10,000 ETH

curl_url() {
  local host="$1"
  local url="$2"
  local -a args=(-fsS)
  if [[ -n "${EDGE_IP}" ]]; then
    args+=(--resolve "${host}:443:${EDGE_IP}")
  fi
  curl "${args[@]}" "${url}"
}

rpc_call() {
  local payload="$1"
  local -a args=(-fsS -H "content-type: application/json")
  if [[ -n "${EDGE_IP}" ]]; then
    args+=(--resolve "${RPC_HOST}:443:${EDGE_IP}")
  fi
  curl "${args[@]}" --data "${payload}" "${RPC_URL}"
}

wait_for_http() {
  local host="$1"
  local url="$2"
  local name="$3"
  local ok=0
  for _ in $(seq 1 "${WAIT_TRIES}"); do
    if curl_url "${host}" "${url}" >/dev/null 2>&1; then
      ok=1
      break
    fi
    sleep "${WAIT_DELAY_SECONDS}"
  done
  if [[ "${ok}" -ne 1 ]]; then
    echo "error: timeout waiting for ${name} at ${url}" >&2
    exit 1
  fi
}

if [[ "${START_STACK}" == "1" ]]; then
  if ! command -v docker >/dev/null 2>&1; then
    echo "error: docker is required when START_STACK=1" >&2
    exit 1
  fi

  echo "Starting demo web surfaces"
  docker compose -f "${ROOT_DIR}/deploy/infra-sites/docker-compose.yml" up -d
  docker compose -f "${ROOT_DIR}/deploy/blockscout/docker-compose.yml" up -d
fi

echo "Checking live endpoints"
wait_for_http "${EXPLORER_HOST}" "${EXPLORER_URL}/api/v2/stats" "explorer api"
wait_for_http "${WALLET_HOST}" "${WALLET_URL}/" "wallet page"
wait_for_http "${MARKET_HOST}" "${MARKET_URL}/" "market page"
wait_for_http "${OBSERVATORY_HOST}" "${OBSERVATORY_URL}/" "observatory page"

chain_id_hex="$(
  rpc_call '{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]}' \
    | jq -r '.result'
)"

if [[ "${chain_id_hex}" != "${EXPECTED_CHAIN_ID_HEX}" ]]; then
  echo "error: chain id mismatch, expected ${EXPECTED_CHAIN_ID_HEX}, got ${chain_id_hex}" >&2
  exit 1
fi

block_a_hex="$(
  rpc_call '{"jsonrpc":"2.0","id":2,"method":"eth_blockNumber","params":[]}' \
    | jq -r '.result'
)"
sleep 3
block_b_hex="$(
  rpc_call '{"jsonrpc":"2.0","id":3,"method":"eth_blockNumber","params":[]}' \
    | jq -r '.result'
)"

if [[ -n "${DEMO_ADDRESS}" ]]; then
  rpc_call "{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"hardhat_setBalance\",\"params\":[\"${DEMO_ADDRESS}\",\"${DEMO_BALANCE_WEI}\"]}" >/dev/null
  funded_balance="$(
    rpc_call "{\"jsonrpc\":\"2.0\",\"id\":5,\"method\":\"eth_getBalance\",\"params\":[\"${DEMO_ADDRESS}\",\"latest\"]}" \
      | jq -r '.result'
  )"
else
  funded_balance=""
fi

rpc_resolve=""
explorer_resolve=""
if [[ -n "${EDGE_IP}" ]]; then
  rpc_resolve="--resolve ${RPC_HOST}:443:${EDGE_IP}"
  explorer_resolve="--resolve ${EXPLORER_HOST}:443:${EDGE_IP}"
fi

cat <<EOF

ETH2077 Investor Demo Mode: READY

Live checks:
- chain id: ${chain_id_hex}
- block number sample A: ${block_a_hex}
- block number sample B: ${block_b_hex}
- explorer: ${EXPLORER_URL}
- wallet:   ${WALLET_URL}
- market:   ${MARKET_URL}
- 2077:     ${OBSERVATORY_URL}
- rpc:      ${RPC_URL}
EOF

if [[ -n "${DEMO_ADDRESS}" ]]; then
  cat <<EOF
- funded address: ${DEMO_ADDRESS}
- funded balance: ${funded_balance}
EOF
fi

cat <<EOF

5-minute live script:
1. Open wallet in browser.
   - URL: ${WALLET_URL}
   - Click "Connect MetaMask".
   - Approve add/switch network if prompted.
   - Confirm UI shows: Network 0x1fb14b (ETH2077).

2. Open network observability board.
   - URL: ${OBSERVATORY_URL}
   - Confirm head slot increments and finalized slot trails by ~2.

3. Send a proof tx.
   - Click "Send 0.001 ETH to Self".
   - Copy tx hash from status line.

4. Show explorer confirmation.
   - Open: ${EXPLORER_URL}/tx/<TX_HASH>
   - Also open blocks page to show liveness:
     ${EXPLORER_URL}/blocks

5. Show app-level transaction.
   - Open ${MARKET_URL}
   - Connect MetaMask
   - Buy any item
   - Open resulting tx in explorer.

6. Close with raw RPC proof commands.
EOF

cat <<EOF

# Chain ID proof
curl -sS ${rpc_resolve} -H 'content-type: application/json' --data '{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]}' ${RPC_URL}

# Latest block proof
curl -sS ${rpc_resolve} -H 'content-type: application/json' --data '{"jsonrpc":"2.0","id":2,"method":"eth_blockNumber","params":[]}' ${RPC_URL}

# Explorer stats proof
curl -sS ${explorer_resolve} ${EXPLORER_URL}/api/v2/stats | jq '{total_blocks, total_transactions, total_addresses}'

# Receipt proof (replace TX_HASH)
curl -sS ${rpc_resolve} -H 'content-type: application/json' --data '{"jsonrpc":"2.0","id":3,"method":"eth_getTransactionReceipt","params":["0xTX_HASH"]}' ${RPC_URL} | jq
EOF
