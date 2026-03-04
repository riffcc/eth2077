#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "usage: $0 <report.json> [report.md]" >&2
  exit 1
fi

JSON_PATH="$1"
MD_PATH="${2:-}"

if [[ ! -f "${JSON_PATH}" ]]; then
  echo "error: missing JSON report: ${JSON_PATH}" >&2
  exit 1
fi

if [[ -n "${MD_PATH}" && ! -f "${MD_PATH}" ]]; then
  echo "error: missing markdown report: ${MD_PATH}" >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

BASE_NAME="$(basename "${JSON_PATH}" .json)"
OUT_DIR="$(dirname "${JSON_PATH}")"
MANIFEST_PATH="${OUT_DIR}/${BASE_NAME}.manifest.json"
CHECKSUM_PATH="${OUT_DIR}/${BASE_NAME}.sha256"
SIG_PATH="${OUT_DIR}/${BASE_NAME}.manifest.sig"
PUBKEY_PATH="${OUT_DIR}/${BASE_NAME}.manifest.pub.pem"

CREATED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
GIT_COMMIT="$(git -C "${ROOT_DIR}" rev-parse HEAD 2>/dev/null || echo unknown)"
if git -C "${ROOT_DIR}" diff --quiet --ignore-submodules HEAD --; then
  GIT_DIRTY=false
else
  GIT_DIRTY=true
fi

HOSTNAME_VALUE="$(hostname 2>/dev/null || echo unknown)"
KERNEL_VALUE="$(uname -srvmo 2>/dev/null || echo unknown)"
RUSTC_VERSION="$(rustc --version 2>/dev/null || echo unknown)"
CARGO_VERSION="$(cargo --version 2>/dev/null || echo unknown)"

JSON_SHA="$(sha256sum "${JSON_PATH}" | awk '{print $1}')"
if [[ -n "${MD_PATH}" ]]; then
  MD_SHA="$(sha256sum "${MD_PATH}" | awk '{print $1}')"
else
  MD_SHA=""
fi

if [[ -n "${MD_PATH}" ]]; then
  sha256sum "${JSON_PATH}" "${MD_PATH}" > "${CHECKSUM_PATH}"
else
  sha256sum "${JSON_PATH}" > "${CHECKSUM_PATH}"
fi

jq -n \
  --arg kind "eth2077-benchmark-artifact-manifest-v1" \
  --arg created_at_utc "${CREATED_AT}" \
  --arg git_commit "${GIT_COMMIT}" \
  --arg hostname "${HOSTNAME_VALUE}" \
  --arg kernel "${KERNEL_VALUE}" \
  --arg rustc "${RUSTC_VERSION}" \
  --arg cargo "${CARGO_VERSION}" \
  --arg json_path "${JSON_PATH}" \
  --arg json_sha256 "${JSON_SHA}" \
  --arg md_path "${MD_PATH}" \
  --arg md_sha256 "${MD_SHA}" \
  --arg checksum_path "${CHECKSUM_PATH}" \
  --arg signing_key_path "${SIGNING_KEY_PATH:-}" \
  --arg rpc_urls "${RPC_URLS:-}" \
  --arg tx_count "${TX_COUNT:-}" \
  --arg tick_ms "${TICK_MS:-}" \
  --arg sender_count "${SENDER_COUNT:-}" \
  --arg gas_limit "${GAS_LIMIT:-}" \
  --argjson git_dirty "${GIT_DIRTY}" \
  '{
    kind: $kind,
    created_at_utc: $created_at_utc,
    git_commit: $git_commit,
    git_dirty: $git_dirty,
    host: {
      hostname: $hostname,
      kernel: $kernel
    },
    toolchain: {
      rustc: $rustc,
      cargo: $cargo
    },
    files: ([
      {
        role: "report_json",
        path: $json_path,
        sha256: $json_sha256
      }
    ] + (if ($md_path|length) > 0 then [{
      role: "report_markdown",
      path: $md_path,
      sha256: $md_sha256
    }] else [] end) + [{
      role: "checksums",
      path: $checksum_path
    }]),
    benchmark_env: {
      rpc_urls: $rpc_urls,
      tx_count: $tx_count,
      tick_ms: $tick_ms,
      sender_count: $sender_count,
      gas_limit: $gas_limit
    },
    signing: {
      key_path: $signing_key_path,
      signature_path: null,
      public_key_path: null,
      verified: false
    }
  }' > "${MANIFEST_PATH}"

if [[ -n "${SIGNING_KEY_PATH:-}" ]]; then
  if [[ ! -f "${SIGNING_KEY_PATH}" ]]; then
    echo "error: SIGNING_KEY_PATH does not exist: ${SIGNING_KEY_PATH}" >&2
    exit 1
  fi

  openssl dgst -sha256 -sign "${SIGNING_KEY_PATH}" -out "${SIG_PATH}" "${MANIFEST_PATH}"
  openssl pkey -in "${SIGNING_KEY_PATH}" -pubout -out "${PUBKEY_PATH}"
  if openssl dgst -sha256 -verify "${PUBKEY_PATH}" -signature "${SIG_PATH}" "${MANIFEST_PATH}" >/dev/null 2>&1; then
    VERIFIED=true
  else
    VERIFIED=false
  fi

  jq \
    --arg sig_path "${SIG_PATH}" \
    --arg pub_path "${PUBKEY_PATH}" \
    --argjson verified "${VERIFIED}" \
    '.signing.signature_path = $sig_path
     | .signing.public_key_path = $pub_path
     | .signing.verified = $verified' \
    "${MANIFEST_PATH}" > "${MANIFEST_PATH}.tmp"
  mv "${MANIFEST_PATH}.tmp" "${MANIFEST_PATH}"
fi

echo "Wrote artifact manifest: ${MANIFEST_PATH}"
echo "Wrote checksums: ${CHECKSUM_PATH}"
if [[ -f "${SIG_PATH}" ]]; then
  echo "Wrote signature: ${SIG_PATH}"
  echo "Wrote public key: ${PUBKEY_PATH}"
fi
