#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd "$SCRIPT_DIR/.." && pwd)
EXAMPLE_DIR="$REPO_ROOT/infra/modules/traefik/examples/basic"

if [[ -z "${TRAEFIK_ACCEPT_E2E_APPLY:-}" ]]; then
  echo "Refusing to run: set TRAEFIK_ACCEPT_E2E_APPLY=1 to allow cluster mutation" >&2
  exit 1
fi

if ! command -v tofu >/dev/null 2>&1; then
  echo "tofu must be installed to run traefik end-to-end checks" >&2
  exit 1
fi

if ! command -v python3 >/dev/null 2>&1; then
  echo "python3 must be installed to run traefik end-to-end checks" >&2
  exit 1
fi

if [[ -z "${TRAEFIK_KUBECONFIG_PATH:-}" ]]; then
  echo "TRAEFIK_KUBECONFIG_PATH must be set to a valid kubeconfig file" >&2
  exit 1
fi

if [[ -z "${TRAEFIK_ACME_EMAIL:-}" ]]; then
  echo "TRAEFIK_ACME_EMAIL must be set (used for the ClusterIssuer)" >&2
  exit 1
fi

if [[ -z "${TRAEFIK_CLOUDFLARE_SECRET_NAME:-}" ]]; then
  echo "TRAEFIK_CLOUDFLARE_SECRET_NAME must be set (secret name holding the Cloudflare token)" >&2
  exit 1
fi

run_id=$(python3 - <<'PY'
import secrets
print(secrets.token_hex(4))
PY
)

workspace="traefik-e2e-${run_id}"
namespace="traefik-e2e-${run_id}"
issuer="issuer-e2e-${run_id}"

# shellcheck disable=SC2329
cleanup_workspace() {
  set +e
  tofu -chdir="$EXAMPLE_DIR" workspace select default >/dev/null 2>&1
  tofu -chdir="$EXAMPLE_DIR" workspace delete "$workspace" >/dev/null 2>&1
}

# shellcheck disable=SC2329
cleanup_resources() {
  set +e
  TF_IN_AUTOMATION=1 tofu -chdir="$EXAMPLE_DIR" destroy -auto-approve -input=false -no-color \
    -var "kubeconfig_path=${TRAEFIK_KUBECONFIG_PATH}" \
    -var "acme_email=${TRAEFIK_ACME_EMAIL}" \
    -var "cloudflare_api_token_secret_name=${TRAEFIK_CLOUDFLARE_SECRET_NAME}" \
    -var "namespace=${namespace}" \
    -var "cluster_issuer_name=${issuer}" >/dev/null 2>&1
}

trap 'cleanup_resources; cleanup_workspace' EXIT

TF_IN_AUTOMATION=1 tofu -chdir="$EXAMPLE_DIR" init -input=false -no-color >/dev/null

tofu -chdir="$EXAMPLE_DIR" workspace new "$workspace" >/dev/null

TF_IN_AUTOMATION=1 tofu -chdir="$EXAMPLE_DIR" apply -auto-approve -input=false -no-color \
  -var "kubeconfig_path=${TRAEFIK_KUBECONFIG_PATH}" \
  -var "acme_email=${TRAEFIK_ACME_EMAIL}" \
  -var "cloudflare_api_token_secret_name=${TRAEFIK_CLOUDFLARE_SECRET_NAME}" \
  -var "namespace=${namespace}" \
  -var "cluster_issuer_name=${issuer}"

plan_status=0
TF_IN_AUTOMATION=1 tofu -chdir="$EXAMPLE_DIR" plan -input=false -no-color -detailed-exitcode \
  -var "kubeconfig_path=${TRAEFIK_KUBECONFIG_PATH}" \
  -var "acme_email=${TRAEFIK_ACME_EMAIL}" \
  -var "cloudflare_api_token_secret_name=${TRAEFIK_CLOUDFLARE_SECRET_NAME}" \
  -var "namespace=${namespace}" \
  -var "cluster_issuer_name=${issuer}" || plan_status=$?

if [[ $plan_status -eq 0 ]]; then
  echo "Traefik e2e: plan is clean after apply (no drift detected)."
  exit 0
fi

if [[ $plan_status -eq 2 ]]; then
  echo "Traefik e2e: plan reported drift after apply (exit code 2)." >&2
  exit 2
fi

echo "Traefik e2e: plan failed (exit code $plan_status)." >&2
exit "$plan_status"
