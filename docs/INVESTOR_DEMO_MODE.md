# ETH2077 Investor Demo Mode

Date: 2026-03-02

## Goal

Run a 5-minute live demo that proves:

1. MetaMask interaction on ETH2077,
2. Live network slot/finality telemetry,
3. Explorer confirmation,
4. Raw RPC correctness checks.

## One Command

```bash
EDGE_IP=10.7.1.200 \
DEMO_ADDRESS=0x375249129507AeC9309ABc1F5c055494200c7c32 \
bash scripts/investor_demo_mode.sh
```

Notes:

1. `EDGE_IP` is optional. Use it when DNS for `*.riff.cc` is not set on the current machine.
2. `DEMO_ADDRESS` is optional. If set, the script tops the account up to `0x21e19e0c9bab2400000` (10,000 ETH) on ETH2077 devnet.
3. Add `START_STACK=1` if you want the script to also run `docker compose up -d` for Blockscout + infra sites before checks.

## 5-Minute Live Flow

1. `00:00-01:00` Wallet connect:
   - Open `https://wallet.riff.cc`
   - Click `Connect MetaMask`
   - Approve add/switch chain prompt
   - Confirm `Network: 0x1fb14b (ETH2077)`

2. `01:00-02:00` Observability proof:
   - Open `https://2077.riff.cc`
   - Confirm head slot increments continuously
   - Confirm finalized slot trails head by ~2 blocks

3. `02:00-03:00` First transaction:
   - Click `Send 0.001 ETH to Self`
   - Copy tx hash from status line

4. `03:00-04:00` Explorer proof:
   - Open `https://explorer.riff.cc/tx/<TX_HASH>`
   - Open `https://explorer.riff.cc/blocks` to show liveness

5. `04:00-05:00` App-level activity:
   - Open `https://market.riff.cc`
   - Click `Connect MetaMask`
   - Buy any item
   - Open resulting tx in explorer

6. `05:00-06:00` Raw RPC close:
   - Run `eth_chainId`
   - Run `eth_blockNumber`
   - Run `eth_getTransactionReceipt` for the tx shown in explorer

## RPC Command Templates

```bash
curl -sS --resolve rpc.riff.cc:443:10.7.1.200 \
  -H 'content-type: application/json' \
  --data '{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]}' \
  https://rpc.riff.cc
```

```bash
curl -sS --resolve rpc.riff.cc:443:10.7.1.200 \
  -H 'content-type: application/json' \
  --data '{"jsonrpc":"2.0","id":2,"method":"eth_getTransactionReceipt","params":["0xTX_HASH"]}' \
  https://rpc.riff.cc | jq
```
