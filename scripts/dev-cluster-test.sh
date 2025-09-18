#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
TF_DIR="$ROOT_DIR/infra/clusters/dev"
POLICY_DIR="$ROOT_DIR/infra/modules/doks/policy"

export TF_VAR_should_create_cluster=true
export TF_IN_AUTOMATION=1

# Static checks
 tofu -chdir="$TF_DIR" fmt -check
 tofu -chdir="$TF_DIR" init -input=false
 tofu -chdir="$TF_DIR" validate

cd "$TF_DIR"
if ! command -v tflint >/dev/null 2>&1; then
  echo "tflint not installed" >&2
fi
tflint --init
tflint --config .tflint.hcl --version
tflint --config .tflint.hcl
cd "$ROOT_DIR"

# Plan and policy tests if token present
trap 'rm -f "$TF_DIR/tfplan.binary" "$TF_DIR/plan.json"' EXIT
if [[ -n "${DIGITALOCEAN_TOKEN:-}" ]]; then
  set +e
  tofu -chdir="$TF_DIR" plan -input=false -out=tfplan.binary -detailed-exitcode
  ec=$?
  set -e
  if [[ $ec -ne 0 && $ec -ne 2 ]]; then
    exit "$ec"
  fi
  tofu -chdir="$TF_DIR" show -json tfplan.binary > "$TF_DIR/plan.json"
  if command -v conftest >/dev/null 2>&1; then
    conftest test "$TF_DIR/plan.json" --policy "$POLICY_DIR"
  else
    echo "Skipping policy: conftest not installed" >&2
  fi
else
  echo "Skipping plan/policy: DIGITALOCEAN_TOKEN not set"
fi

# Go tests
cd "$ROOT_DIR/infra/clusters/dev/tests"
DOKS_KUBERNETES_VERSION="${DOKS_KUBERNETES_VERSION:-}" go test -v
