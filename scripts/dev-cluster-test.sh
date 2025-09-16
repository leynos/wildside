#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
TF_DIR="$ROOT_DIR/infra/clusters/dev"
POLICY_DIR="$ROOT_DIR/infra/modules/doks/policy"

DOKS_VERSION=${DOKS_KUBERNETES_VERSION:-}
if [[ -n "$DOKS_VERSION" ]]; then
  export TF_VAR_kubernetes_version="$DOKS_VERSION"
else
  unset TF_VAR_kubernetes_version
fi
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
if [[ -n "${DIGITALOCEAN_TOKEN:-}" ]]; then
  tofu -chdir="$TF_DIR" plan -out=tfplan.binary -detailed-exitcode || [ $? -eq 2 ]
  tofu -chdir="$TF_DIR" show -json tfplan.binary > "$TF_DIR/plan.json"
  conftest test "$TF_DIR/plan.json" --policy "$POLICY_DIR"
  rm -f "$TF_DIR/tfplan.binary" "$TF_DIR/plan.json"
else
  echo "Skipping plan/policy: DIGITALOCEAN_TOKEN not set"
fi

# Go tests
cd "$ROOT_DIR/infra/clusters/dev/tests"
if [[ -n "$DOKS_VERSION" ]]; then
  DOKS_KUBERNETES_VERSION=$DOKS_VERSION go test -v
else
  go test -v
fi
