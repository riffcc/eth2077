# ETH2077 Local Devnet

Date: 2026-03-02

## Purpose

Run a deterministic multi-node ETH2077 devnet locally for development and integration testing.

This is a local deployment profile, not a production/security hardening profile.

## One-Command Deploy

```bash
bash scripts/devnet_up.sh
```

Default settings:

- `NODE_COUNT=4`
- `NETWORK=eth2077-devnet-local`
- `CHAIN_ID=2077002`
- `RPC_BASE_PORT=9545` (nodes become `9545..`)
- `P2P_BASE_PORT=30303` (nodes become `30303..`)

## Status

```bash
bash scripts/devnet_status.sh
```

## Shutdown

```bash
bash scripts/devnet_down.sh
```

## Optional Overrides

```bash
NODE_COUNT=8 \
CHAIN_ID=2077010 \
NETWORK=eth2077-devnet-8n \
bash scripts/devnet_up.sh
```

Other environment overrides:

- `DEVNET_DIR`
- `SEED`
- `GENESIS_TS`
- `FORK_EPOCH`
- `ALLOC_ACCOUNTS`
- `BOOTNODES`
- `TICK_MS`

## Engine API Stub Endpoints

Each node exposes local HTTP endpoints:

- `GET /healthz`
- `GET /status`
- `GET /engine/v1/capabilities`
- `POST /engine/v1/newPayloadV3`
- `POST /engine/v1/forkchoiceUpdatedV3`

Engine API paths above remain compatibility stubs for devnet workflows.

JSON-RPC execution path now runs real EVM execution for contract deployment/calls and transaction processing (`eth_sendRawTransaction`, `eth_call`, `eth_getCode`, `eth_getStorageAt`, `eth_getTransactionReceipt`, `eth_getLogs`, `eth_estimateGas`).
