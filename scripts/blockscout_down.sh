#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
COMPOSE_DIR="${ROOT_DIR}/deploy/blockscout"

if [[ ! -f "${COMPOSE_DIR}/docker-compose.yml" ]]; then
  echo "No blockscout compose file found at ${COMPOSE_DIR}/docker-compose.yml"
  exit 1
fi

cd "${COMPOSE_DIR}"
docker compose down
echo "Blockscout stack stopped"
