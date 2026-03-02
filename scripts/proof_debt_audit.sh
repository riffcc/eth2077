#!/usr/bin/env bash
set -euo pipefail

if ! command -v rg >/dev/null 2>&1; then
  echo "error: rg is required" >&2
  exit 1
fi

CITADEL_ROOT="${CITADEL_ROOT:-/mnt/riffcastle/lagun-project/citadel/proofs}"
LAGOON_ROOT="${LAGOON_ROOT:-/mnt/riffcastle/lagun-project/lagoon/proofs}"
JSON_MODE="${1:-}"

count_token() {
  local token="$1"
  local root="$2"
  (rg -o --no-messages --glob '*.lean' "\\b${token}\\b" "$root" || true) | wc -l | tr -d ' '
}

count_by_file() {
  local token="$1"
  local root="$2"
  (rg -n --no-messages --glob '*.lean' "\\b${token}\\b" "$root" || true) \
    | cut -d: -f1 \
    | sort \
    | uniq -c \
    | sort -nr
}

citadel_sorry="$(count_token sorry "$CITADEL_ROOT")"
citadel_axiom="$(count_token axiom "$CITADEL_ROOT")"
lagoon_sorry="$(count_token sorry "$LAGOON_ROOT")"
lagoon_axiom="$(count_token axiom "$LAGOON_ROOT")"

timestamp="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

if [[ "$JSON_MODE" == "--json" ]]; then
  cat <<JSON
{
  "timestamp_utc": "${timestamp}",
  "roots": {
    "citadel": "${CITADEL_ROOT}",
    "lagoon": "${LAGOON_ROOT}"
  },
  "counts": {
    "citadel": {"sorry": ${citadel_sorry}, "axiom": ${citadel_axiom}},
    "lagoon": {"sorry": ${lagoon_sorry}, "axiom": ${lagoon_axiom}}
  }
}
JSON
  exit 0
fi

cat <<TEXT
ETH2077 proof-debt audit
Timestamp (UTC): ${timestamp}

Roots:
- citadel: ${CITADEL_ROOT}
- lagoon: ${LAGOON_ROOT}

Summary:
- citadel: sorry=${citadel_sorry}, axiom=${citadel_axiom}
- lagoon:  sorry=${lagoon_sorry}, axiom=${lagoon_axiom}

Top citadel files by 'sorry':
TEXT
count_by_file sorry "$CITADEL_ROOT" | head -n 10 || true

cat <<TEXT

Top citadel files by 'axiom':
TEXT
count_by_file axiom "$CITADEL_ROOT" | head -n 10 || true

cat <<TEXT

Top lagoon files by 'sorry':
TEXT
count_by_file sorry "$LAGOON_ROOT" | head -n 10 || true

cat <<TEXT

Top lagoon files by 'axiom':
TEXT
count_by_file axiom "$LAGOON_ROOT" | head -n 10 || true
