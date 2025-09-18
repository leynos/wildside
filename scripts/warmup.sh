#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

log() {
  printf '[warmup] %s\n' "$*"
}

# Initialise TFLint plugins wherever a configuration file exists.
if command -v tflint >/dev/null 2>&1; then
  mapfile -d '' -t TFLINT_CONFIGS < <(find "$ROOT_DIR" -name '.tflint.hcl' -print0)
  if ((${#TFLINT_CONFIGS[@]})); then
    for cfg in "${TFLINT_CONFIGS[@]}"; do
      dir=$(dirname "$cfg")
      rel=${dir#"$ROOT_DIR/"}
      log "Initialising TFLint plugins in ${rel:-.}"
      (cd "$dir" && tflint --init >/dev/null)
    done
  else
    log "No .tflint.hcl files found; skipping TFLint warmup"
  fi
else
  log "tflint not available; skipping TFLint warmup"
fi

# Pre-download providers for Terraform/OpenTofu modules.
if command -v tofu >/dev/null 2>&1; then
  mapfile -t TF_DIRS < <(find "$ROOT_DIR/infra" -type f -name '*.tf' -printf '%h\n' | sort -u)
  if ((${#TF_DIRS[@]})); then
    for dir in "${TF_DIRS[@]}"; do
      rel=${dir#"$ROOT_DIR/"}
      log "Running 'tofu init' in ${rel}"
      tofu -chdir="$dir" init -backend=false -upgrade=false >/dev/null || true
    done
  else
    log "No Terraform modules found under infra; skipping tofu warmup"
  fi
else
  log "OpenTofu not available; skipping Terraform warmup"
fi

# Prime Terratest Go modules so later runs are quicker.
if command -v go >/dev/null 2>&1; then
  mapfile -t GO_MODULES < <(find "$ROOT_DIR/infra" -name 'go.mod' -printf '%h\n')
  if ((${#GO_MODULES[@]})); then
    for dir in "${GO_MODULES[@]}"; do
      rel=${dir#"$ROOT_DIR/"}
      log "Warming Go module in ${rel}"
      pushd "$dir" >/dev/null
      go mod download all >/dev/null 2>&1 || true
      go list ./... 2>/dev/null | xargs -r -I {} bash -c 'go build -v "${1}" >/dev/null 2>&1 || true' _ {}
      go list -test ./... 2>/dev/null | xargs -r -I {} bash -c 'go test -c "${1}" -o /dev/null >/dev/null 2>&1 || true' _ {}
      popd >/dev/null
    done
  else
    log "No Go modules found under infra; skipping Terratest warmup"
  fi
else
  log "Go toolchain not available; skipping Terratest warmup"
fi

log "Warmup complete"
