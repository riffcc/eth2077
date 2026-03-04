# Blockscout on ETH2077 Devnet

Date: 2026-03-02

## Purpose

Deploy Blockscout against an integrated ETH2077 Docker devnet for explorer/API workflows.

## Start

```bash
bash scripts/blockscout_up.sh
```

This script:

1. Builds and launches `eth2077-devnet` service,
2. Deploys Blockscout backend + frontend + postgres + redis via Docker Compose,
3. Exposes:
   - devnet rpc: `http://127.0.0.1:8545`
   - frontend: `http://127.0.0.1:3000`
   - backend: `http://127.0.0.1:4000`

## Stop

```bash
bash scripts/blockscout_down.sh
```

## Optional Overrides

```bash
CHAIN_ID=2077003 \
DISABLE_INDEXER=false \
NEXT_PUBLIC_APP_HOST=10.7.1.195 \
NEXT_PUBLIC_API_HOST=10.7.1.195 \
bash scripts/blockscout_up.sh
```

## Notes

1. Default profile uses `DISABLE_INDEXER=false` so Blockscout can index blocks.
2. Frontend ads are disabled by default via:
   - `NEXT_PUBLIC_AD_BANNER_PROVIDER=none`
   - `NEXT_PUBLIC_AD_TEXT_PROVIDER=none`
3. For LAN access, set both `NEXT_PUBLIC_APP_HOST` and `NEXT_PUBLIC_API_HOST` to your host IP.
4. The ETH2077 devnet node now exposes a real EVM execution JSON-RPC lane for contract deploy/call flows, while Engine API endpoints remain devnet compatibility stubs.
