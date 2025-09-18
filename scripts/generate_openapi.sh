#!/usr/bin/env bash
# shellcheck disable=SC2312
# Generate the OpenAPI document via the backend dumper and sort keys with jq.
# Accepts the target path as an optional argument.
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
ROOT_DIR=$(cd "${SCRIPT_DIR}/.." && pwd)
TARGET=${1:-spec/openapi.json}
TMP_FILE=$(mktemp "${ROOT_DIR}/${TARGET}.tmp.XXXXXX")

cleanup() {
  rm -f "${TMP_FILE}"
}
trap cleanup EXIT

cargo run --quiet --manifest-path "${ROOT_DIR}/backend/Cargo.toml" --bin openapi-dump > "${TMP_FILE}"
jq -S . "${TMP_FILE}" > "${ROOT_DIR}/${TARGET}"
trap - EXIT
