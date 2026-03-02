#!/usr/bin/env bash
set -euo pipefail

if ! command -v jq >/dev/null 2>&1; then
  echo "error: jq is required" >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
DATE_TAG="$(date -u +%F)"

BENCH_JSON="${ROOT_DIR}/reports/testnet-go-nogo-bench-${DATE_TAG}.json"
BENCH_MD="${ROOT_DIR}/reports/testnet-go-nogo-bench-${DATE_TAG}.md"
REPORT_MD="${ROOT_DIR}/reports/testnet-go-nogo-${DATE_TAG}.md"
ARTIFACT_DIR="${ROOT_DIR}/artifacts/testnet-alpha"

echo "Running ETH2077 testnet go/no-go checks (${DATE_TAG})"

FORMAL_OUTPUT="$(bash "${ROOT_DIR}/scripts/check_eth2077_formal_gates.sh" --require-proofs)"

BENCH_OUTPUT="$(cargo run -p eth2077-bench --release -- \
  --scenario-set default \
  --seed 2077 \
  --tx-count 600000 \
  --output-json "${BENCH_JSON}" \
  --output-md "${BENCH_MD}" 2>&1)"

ARTIFACT_OUTPUT="$(bash "${ROOT_DIR}/scripts/build_testnet_artifacts.sh" 2>&1)"

MAX_TPS="$(jq '[.[].sustained_tps] | max' "${BENCH_JSON}")"
MAX_P99_MS="$(jq '[.[].p99_finality_ms] | max' "${BENCH_JSON}")"

if grep -q "PASS: no placeholder debt detected" <<<"${FORMAL_OUTPUT}" && \
  [[ -f "${ARTIFACT_DIR}/SHA256SUMS" ]] && \
  jq -e '. >= 1000000' <<<"${MAX_TPS}" >/dev/null; then
  VERDICT="PASS"
else
  VERDICT="FAIL"
fi

cat > "${REPORT_MD}" <<EOF
# ETH2077 Testnet Go/No-Go (${DATE_TAG})

## Summary

- verdict: **${VERDICT}**
- max sustained TPS (default deterministic suite): **$(printf '%.0f' "${MAX_TPS}")**
- worst-case p99 finality (ms): **$(printf '%.1f' "${MAX_P99_MS}")**
- artifacts dir: \`${ARTIFACT_DIR}\`

## Formal Gate Output

\`\`\`
${FORMAL_OUTPUT}
\`\`\`

## Benchmark Command Output (tail)

\`\`\`
$(tail -n 20 <<<"${BENCH_OUTPUT}")
\`\`\`

## Artifact Builder Output (tail)

\`\`\`
$(tail -n 20 <<<"${ARTIFACT_OUTPUT}")
\`\`\`

## Required Files

- \`${BENCH_JSON}\`
- \`${BENCH_MD}\`
- \`${ARTIFACT_DIR}/chain-spec.json\`
- \`${ARTIFACT_DIR}/genesis.json\`
- \`${ARTIFACT_DIR}/metadata.json\`
- \`${ARTIFACT_DIR}/bootnodes.txt\`
- \`${ARTIFACT_DIR}/SHA256SUMS\`
EOF

echo "Wrote go/no-go report: ${REPORT_MD}"
if [[ "${VERDICT}" != "PASS" ]]; then
  exit 1
fi
