#!/usr/bin/env bash
set -euo pipefail

if ! command -v rg >/dev/null 2>&1; then
  echo "error: rg is required" >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
PROOFS_DIR="${PROOFS_DIR:-${ROOT_DIR}/proofs}"
REQUIRE_PROOFS="false"

for arg in "$@"; do
  case "$arg" in
    --require-proofs)
      REQUIRE_PROOFS="true"
      ;;
    *)
      echo "error: unknown argument '$arg'" >&2
      echo "usage: $0 [--require-proofs]" >&2
      exit 2
      ;;
  esac
done

if [[ ! -d "$PROOFS_DIR" ]]; then
  if [[ "$REQUIRE_PROOFS" == "true" ]]; then
    echo "FAIL: proofs directory missing at $PROOFS_DIR"
    exit 1
  fi
  echo "WARN: proofs directory missing at $PROOFS_DIR (allowed in non-strict mode)"
  exit 0
fi

lean_count="$(find "$PROOFS_DIR" -type f -name '*.lean' | wc -l | tr -d ' ')"
if [[ "$lean_count" == "0" ]]; then
  if [[ "$REQUIRE_PROOFS" == "true" ]]; then
    echo "FAIL: no Lean proof files found under $PROOFS_DIR"
    exit 1
  fi
  echo "WARN: no Lean proof files found under $PROOFS_DIR (allowed in non-strict mode)"
  exit 0
fi

sorry_count="$(rg -o --glob '*.lean' '\bsorry\b' "$PROOFS_DIR" | wc -l | tr -d ' ')"
axiom_count="$(rg -o --glob '*.lean' '\baxiom\b' "$PROOFS_DIR" | wc -l | tr -d ' ')"

echo "ETH2077 local formal gate"
echo "- proofs_dir: $PROOFS_DIR"
echo "- lean_files: $lean_count"
echo "- sorry: $sorry_count"
echo "- axiom: $axiom_count"

if [[ "$sorry_count" != "0" || "$axiom_count" != "0" ]]; then
  echo "FAIL: critical-path placeholders detected"
  exit 1
fi

echo "PASS: no placeholder debt detected"
