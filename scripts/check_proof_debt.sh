#!/usr/bin/env bash
set -euo pipefail

proof_dir="proofs"

if [ ! -d "$proof_dir" ]; then
  echo "No proofs directory found at '$proof_dir'; nothing to check."
  exit 0
fi

matches="$(find "$proof_dir" -type f -name '*.lean' -exec grep -nH -w 'sorry' {} + 2>/dev/null || true)"

if [ -n "$matches" ]; then
  echo "Proof debt detected: found 'sorry' placeholders in Lean files:"
  printf '%s\n' "$matches" | awk -F: '{print $1 ":" $2}'
  exit 1
fi

echo "Proof debt check passed: no 'sorry' placeholders found."
exit 0
