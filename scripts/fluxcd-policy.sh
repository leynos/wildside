#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd "$SCRIPT_DIR/.." && pwd)
EXAMPLE_DIR="$REPO_ROOT/infra/modules/fluxcd/examples/basic"
POLICY_DIR="$REPO_ROOT/infra/modules/fluxcd/policy"
PLAN_BINARY="$EXAMPLE_DIR/tfplan.binary"

if [[ -z "${FLUX_KUBECONFIG_PATH:-}" ]]; then
  echo "FLUX_KUBECONFIG_PATH must be provided for fluxcd-policy" >&2
  exit 1
fi

GIT_REPOSITORY_URL="${FLUX_GIT_REPOSITORY_URL:-https://github.com/fluxcd/flux2-kustomize-helm-example.git}"
GIT_REPOSITORY_PATH="${FLUX_GIT_REPOSITORY_PATH:-./clusters/my-cluster}"
GIT_REPOSITORY_BRANCH="${FLUX_GIT_REPOSITORY_BRANCH:-main}"

plan_json=$(mktemp)
tmp_data=""
cleanup() {
  rm -f "$plan_json" "$PLAN_BINARY"
  if [[ -n "$tmp_data" ]]; then
    rm -f "$tmp_data"
  fi
}
trap cleanup EXIT

declare -a data_args=()
if [[ -n "${FLUX_POLICY_PARAMS_JSON:-}" ]]; then
  tmp_data=$(mktemp)
  printf '%s' "${FLUX_POLICY_PARAMS_JSON}" > "$tmp_data"
  data_args+=(-d "$tmp_data")
elif [[ -n "${FLUX_POLICY_DATA:-}" ]]; then
  data_args+=(-d "${FLUX_POLICY_DATA}")
fi

plan_status=0
TF_IN_AUTOMATION=1 tofu -chdir="$EXAMPLE_DIR" plan -input=false -no-color -out=tfplan.binary -detailed-exitcode \
  -var "git_repository_url=${GIT_REPOSITORY_URL}" \
  -var "git_repository_path=${GIT_REPOSITORY_PATH}" \
  -var "git_repository_branch=${GIT_REPOSITORY_BRANCH}" \
  -var "kubeconfig_path=${FLUX_KUBECONFIG_PATH}" || plan_status=$?
if [[ $plan_status -ne 0 && $plan_status -ne 2 ]]; then
  exit "$plan_status"
fi

if ! TF_IN_AUTOMATION=1 tofu -chdir="$EXAMPLE_DIR" show -json tfplan.binary > "$plan_json"; then
  exit $?
fi

conftest test "$plan_json" --policy "$POLICY_DIR" "${data_args[@]}"
