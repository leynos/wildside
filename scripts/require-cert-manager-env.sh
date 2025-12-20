#!/usr/bin/env bash
set -euo pipefail

if [[ -z "${CERT_MANAGER_KUBECONFIG_PATH:-}" ]]; then
  exit 0
fi

required_vars=(
  CERT_MANAGER_ACME_EMAIL
  CERT_MANAGER_NAMECHEAP_SECRET_NAME
  CERT_MANAGER_VAULT_SERVER
  CERT_MANAGER_VAULT_PKI_PATH
  CERT_MANAGER_VAULT_TOKEN_SECRET_NAME
  CERT_MANAGER_VAULT_CA_BUNDLE_PEM
)

missing=()
for var in "${required_vars[@]}"; do
  if [[ -z "${!var:-}" ]]; then
    missing+=("$var")
  fi
done

if [[ ${#missing[@]} -ne 0 ]]; then
  echo "CERT_MANAGER_ACME_EMAIL, CERT_MANAGER_NAMECHEAP_SECRET_NAME, CERT_MANAGER_VAULT_SERVER, CERT_MANAGER_VAULT_PKI_PATH, CERT_MANAGER_VAULT_TOKEN_SECRET_NAME, and CERT_MANAGER_VAULT_CA_BUNDLE_PEM must be set when CERT_MANAGER_KUBECONFIG_PATH is set" >&2
  exit 1
fi
