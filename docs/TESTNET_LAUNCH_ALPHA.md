# ETH2077 Alpha Testnet Launch Spec (v0)

Date: 2026-03-02

## Frozen Launch Constants

- network: `eth2077-alpha`
- chain-id: `2077001`
- validator count: `48`
- byzantine fault budget (`f`): `15`
- quorum threshold: `31`
- default genesis timestamp: `1772409600` (UTC)
- default fork epoch: `0`

These values are enforced by:

- deterministic artifact generator:
  - `cargo run -p eth2077-testnet --release -- --seed 2077`
- proof gates:
  - `proofs/ETH2077Proofs/TestnetGates.lean`

## Artifact Build

```bash
bash scripts/build_testnet_artifacts.sh
```

Expected outputs under `artifacts/testnet-alpha/`:

- `chain-spec.json`
- `genesis.json`
- `metadata.json`
- `bootnodes.txt`
- `SHA256SUMS`

## Go/No-Go Command

```bash
bash scripts/check_testnet_go_nogo.sh
```

The launch gate marks `PASS` only when:

1. formal gate passes (`0` placeholder debt in local proofs),
2. deterministic benchmark suite reaches `>= 1,000,000` max sustained TPS,
3. deterministic testnet artifacts + checksums are present.

## Safety Notes

1. Generated keys and bootnodes are deterministic and intended for dev/testnet use only.
2. This is not a production key ceremony flow.
3. Production launch requires HSM/remote signer controls and threat model closure.
