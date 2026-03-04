#!/usr/bin/env bash
set -euo pipefail

if ! command -v jq >/dev/null 2>&1; then
  echo "error: jq is required" >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
DATE_TAG="$(date -u +%F)"

EIP_TAG="${1:-eip-unknown}"
SEED="${SEED:-2077}"
TX_COUNT="${TX_COUNT:-200000}"
SCENARIO_SET="${SCENARIO_SET:-default}"

BENCH_JSON="${ROOT_DIR}/reports/eth2077-mesh-bench-${DATE_TAG}-${EIP_TAG}.json"
BENCH_MD="${ROOT_DIR}/reports/eth2077-mesh-bench-${DATE_TAG}-${EIP_TAG}.md"
REPORT_MD="${ROOT_DIR}/reports/eip-step-gate-${DATE_TAG}-${EIP_TAG}.md"

echo "Running ETH2077 EIP step gate (${EIP_TAG})"

TEST_OUTPUT="$(cargo test -p eth2077-node --bin eth2077-devnetd 2>&1)"
FORMAL_OUTPUT="$(bash "${ROOT_DIR}/scripts/check_eth2077_formal_gates.sh" --require-proofs 2>&1)"
BENCH_OUTPUT="$(cargo run -p eth2077-bench -- \
  --scenario-set "${SCENARIO_SET}" \
  --seed "${SEED}" \
  --tx-count "${TX_COUNT}" \
  --output-json "${BENCH_JSON}" \
  --output-md "${BENCH_MD}" 2>&1)"

MAX_TPS="$(jq '[.[].sustained_tps] | max' "${BENCH_JSON}")"
MAX_P99_MS="$(jq '[.[].p99_finality_ms] | max' "${BENCH_JSON}")"
BOTTLENECK="$(jq -r 'max_by(.sustained_tps).bottleneck' "${BENCH_JSON}")"

VERDICT="PASS"
if ! grep -q "test result: ok" <<<"${TEST_OUTPUT}"; then
  VERDICT="FAIL"
fi
if ! grep -q "PASS: no placeholder debt detected" <<<"${FORMAL_OUTPUT}"; then
  VERDICT="FAIL"
fi
if ! jq -e '. >= 1' <<<"${MAX_TPS}" >/dev/null; then
  VERDICT="FAIL"
fi

cat > "${REPORT_MD}" <<EOF
# ETH2077 EIP Step Gate (${EIP_TAG}) - ${DATE_TAG}

## Summary

- verdict: **${VERDICT}**
- scenario_set: \`${SCENARIO_SET}\`
- seed: \`${SEED}\`
- tx_count per scenario: \`${TX_COUNT}\`
- max sustained TPS: **$(printf '%.0f' "${MAX_TPS}")**
- worst-case p99 finality (ms): **$(printf '%.1f' "${MAX_P99_MS}")**
- top-scenario bottleneck: **${BOTTLENECK}**

## Test Output (tail)

\`\`\`
$(tail -n 40 <<<"${TEST_OUTPUT}")
\`\`\`

## Formal Gate Output

\`\`\`
${FORMAL_OUTPUT}
\`\`\`

## Benchmark Output (tail)

\`\`\`
$(tail -n 40 <<<"${BENCH_OUTPUT}")
\`\`\`

## Artifacts

- \`${BENCH_JSON}\`
- \`${BENCH_MD}\`
- \`${REPORT_MD}\`
EOF

echo "Wrote EIP step gate report: ${REPORT_MD}"
if [[ "${VERDICT}" != "PASS" ]]; then
  exit 1
fi

