#!/usr/bin/env bash
set -euo pipefail

if ! command -v docker >/dev/null 2>&1; then
  echo "error: docker is required" >&2
  exit 1
fi

if ! command -v curl >/dev/null 2>&1; then
  echo "error: curl is required" >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
COMPOSE_DIR="${ROOT_DIR}/deploy/blockscout"
CHAIN_ID="${CHAIN_ID:-2077003}"
DISABLE_INDEXER="${DISABLE_INDEXER:-false}"
LAN_IP_DETECTED="$(ip route get 1.1.1.1 2>/dev/null | awk '{for(i=1;i<=NF;i++) if($i=="src"){print $(i+1); exit}}')"
if [[ -z "${LAN_IP_DETECTED}" ]]; then
  LAN_IP_DETECTED="$(hostname -I 2>/dev/null | awk '{print $1}')"
fi
# Default to the DNS host so TLS wildcard certs (*.riff.cc) stay valid.
# Set NEXT_PUBLIC_APP_HOST/NEXT_PUBLIC_API_HOST explicitly for raw LAN/IP runs.
APP_HOST="${NEXT_PUBLIC_APP_HOST:-explorer.riff.cc}"
API_HOST="${NEXT_PUBLIC_API_HOST:-${APP_HOST}}"

echo "Starting Blockscout stack"
cd "${COMPOSE_DIR}"

echo "Building eth2077/devnet:local image"
DOCKER_BUILDKIT=0 docker build \
  -f "${COMPOSE_DIR}/Dockerfile.devnet" \
  -t eth2077/devnet:local \
  "${ROOT_DIR}"

CHAIN_ID="${CHAIN_ID}" \
DISABLE_INDEXER="${DISABLE_INDEXER}" \
NEXT_PUBLIC_APP_HOST="${APP_HOST}" \
NEXT_PUBLIC_API_HOST="${API_HOST}" \
docker compose up -d

echo "Waiting for backend health"
for _ in {1..60}; do
  if curl -fsS "http://127.0.0.1:4000/api/v2/main-page/blocks" >/dev/null 2>&1; then
    break
  fi
  sleep 2
done

echo "Blockscout deployment status"
docker compose ps
echo "Devnet RPC: http://127.0.0.1:8545"
echo "Frontend: http://127.0.0.1:3000"
echo "Backend:  http://127.0.0.1:4000"
if [[ "${APP_HOST}" != "localhost" ]]; then
  echo "LAN Frontend: http://${APP_HOST}:3000"
  echo "LAN RPC:      http://${APP_HOST}:8545"
  echo "LAN Backend:  http://${APP_HOST}:4000"
fi
if [[ "${APP_HOST}" =~ ^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  cat <<EOF
warning: NEXT_PUBLIC_APP_HOST is an IP (${APP_HOST}).
If your browser is using HTTPS/WSS, certificate validation may fail (CN mismatch).
Prefer DNS hostnames covered by your cert, e.g. explorer.riff.cc.
EOF
fi
