#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: $0 <artifact.manifest.json>" >&2
  exit 1
fi

require_cmd() {
  local cmd="$1"
  if ! command -v "${cmd}" >/dev/null 2>&1; then
    echo "error: required command not found: ${cmd}" >&2
    exit 1
  fi
}

require_cmd jq
require_cmd sha256sum

MANIFEST_PATH="$1"
if [[ ! -f "${MANIFEST_PATH}" ]]; then
  echo "error: missing manifest: ${MANIFEST_PATH}" >&2
  exit 1
fi

MANIFEST_DIR="$(cd "$(dirname "${MANIFEST_PATH}")" && pwd)"
MANIFEST_BASENAME="$(basename "${MANIFEST_PATH}")"
MANIFEST_STEM="${MANIFEST_PATH%.json}"
SIG_PATH="${MANIFEST_STEM}.sig"
PUBKEY_PATH="${MANIFEST_STEM}.pub.pem"

if ! jq -e '
  (.kind | type == "string" and length > 0) and
  (.git_commit | type == "string" and length > 0) and
  (.created_at_utc | type == "string" and length > 0) and
  (.git_dirty | type == "boolean") and
  (.host | type == "object") and
  (.toolchain | type == "object") and
  (.files | type == "array" and length > 0) and
  (.benchmark_env | type == "object") and
  (.signing | type == "object")
' "${MANIFEST_PATH}" >/dev/null; then
  echo "error: manifest failed schema validation: ${MANIFEST_PATH}" >&2
  exit 1
fi

echo "Manifest schema: PASS (${MANIFEST_BASENAME})"

files_total=0
files_verified=0
files_failed=0

while IFS=$'\t' read -r role file_path expected_sha; do
  [[ -n "${file_path}" ]] || continue
  [[ -n "${expected_sha}" ]] || continue

  files_total=$((files_total + 1))

  resolved_path="${file_path}"
  if [[ "${resolved_path}" != /* && ! -f "${resolved_path}" && -f "${MANIFEST_DIR}/${resolved_path}" ]]; then
    resolved_path="${MANIFEST_DIR}/${resolved_path}"
  fi

  if [[ ! "${expected_sha}" =~ ^[0-9a-fA-F]{64}$ ]]; then
    echo "FAIL checksum (${role:-unknown}): ${file_path} (invalid sha256 in manifest)"
    files_failed=$((files_failed + 1))
    continue
  fi

  if [[ ! -f "${resolved_path}" ]]; then
    echo "FAIL checksum (${role:-unknown}): ${file_path} (missing file)"
    files_failed=$((files_failed + 1))
    continue
  fi

  actual_sha="$(sha256sum "${resolved_path}" | awk '{print $1}')"
  if [[ "${actual_sha}" == "${expected_sha}" ]]; then
    echo "PASS checksum (${role:-unknown}): ${file_path}"
    files_verified=$((files_verified + 1))
  else
    echo "FAIL checksum (${role:-unknown}): ${file_path} (expected ${expected_sha}, got ${actual_sha})"
    files_failed=$((files_failed + 1))
  fi
done < <(
  jq -r '.files[]?
    | select((.path? | type == "string") and (.path | length > 0))
    | select((.sha256? | type == "string") and (.sha256 | length > 0))
    | [(.role // ""), .path, .sha256]
    | @tsv' "${MANIFEST_PATH}"
)

if [[ "${files_total}" -eq 0 ]]; then
  echo "FAIL checksum: no hashable file entries found in manifest"
  files_failed=$((files_failed + 1))
fi

signature_status="not-signed"
signature_ok=true

if [[ -f "${SIG_PATH}" ]]; then
  require_cmd openssl
  if [[ ! -f "${PUBKEY_PATH}" ]]; then
    echo "FAIL signature: missing public key: ${PUBKEY_PATH}"
    signature_status="failed"
    signature_ok=false
  elif openssl dgst -sha256 -verify "${PUBKEY_PATH}" -signature "${SIG_PATH}" "${MANIFEST_PATH}" >/dev/null 2>&1; then
    echo "PASS signature: ${SIG_PATH}"
    signature_status="verified"
  else
    echo "FAIL signature: verification failed for ${SIG_PATH}"
    signature_status="failed"
    signature_ok=false
  fi
elif [[ -f "${PUBKEY_PATH}" ]]; then
  echo "FAIL signature: found public key without signature: ${PUBKEY_PATH}"
  signature_status="failed"
  signature_ok=false
else
  echo "WARN signature: not signed (no ${SIG_PATH} / ${PUBKEY_PATH})"
fi

if [[ "${files_failed}" -eq 0 && "${signature_ok}" == true ]]; then
  echo "Verification PASSED: ${files_verified}/${files_total} files verified, signature: ${signature_status}"
  exit 0
fi

echo "Verification FAILED: ${files_verified}/${files_total} files verified, signature: ${signature_status}" >&2
exit 1
