# ETH2077 Network Observatory (`2077.riff.cc`)

Date: 2026-03-02

## Goal

Ship a dedicated ETH2077 operations board that looks and behaves like a lightweight Grafana panel wall for live demo and debugging.

## URL

- `https://2077.riff.cc`

## What It Shows

- Live head slot and finalized slot display.
- Finality lag in blocks and ms estimate.
- RPC latency and observed block interval.
- Latest block tx count and rolling throughput approximation.
- Pending tx queue and known tx cache count.
- Recent block table (height, age, txs, gas, hash).
- Raw status snapshot for quick debug inspection.

## Data Sources

- Standard RPC:
  - `web3_clientVersion`
  - `eth_chainId`
  - `eth_blockNumber`
  - `eth_getBlockByNumber`
  - `eth_getBlockTransactionCountByNumber`
  - `net_peerCount`
- ETH2077 custom RPC:
  - `eth2077_status`

## Local Service Wiring

- Static site directory: `deploy/infra-sites/2077`
- Docker service: `dashboard-site` in `deploy/infra-sites/docker-compose.yml`
- Local bind: `127.0.0.1:3103 -> nginx:80`

## Bring Up / Refresh

```bash
docker compose -f deploy/infra-sites/docker-compose.yml up -d --force-recreate dashboard-site
```

## Edge Routing Note

To expose the page at `https://2077.riff.cc`, route that host in your TLS edge proxy (HAProxy/Nginx) to `127.0.0.1:3103`, matching the pattern already used for `wallet.riff.cc` and `market.riff.cc`.
