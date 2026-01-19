#!/usr/bin/env -S uv run python
# /// script
# requires-python = ">=3.13"
# dependencies = ["cyclopts>=2.9"]
# ///
"""Prepare inputs for the wildside-infra-k8s GitHub Action.

This command resolves CLI and environment inputs, validates them, masks
sensitive values (via ``prepare_inputs`` and ``mask_secret``), and exports
resolved values to ``GITHUB_ENV`` for downstream steps.

Examples
--------
Run with explicit CLI overrides:

>>> python scripts/prepare_infra_k8s_inputs.py --cluster-name preview-1 --region nyc1
"""

from __future__ import annotations

from collections.abc import Mapping
from pathlib import Path

from cyclopts import App, Parameter
from scripts._prepare_infra_k8s_inputs import (
    RawInputs,
    ResolvedInputs,
    _resolve_all_inputs,
    prepare_inputs,
)

__all__ = [
    "RawInputs",
    "ResolvedInputs",
    "prepare_inputs",
]

app = App(help="Prepare wildside-infra-k8s action inputs.")

CLUSTER_NAME_PARAM = Parameter()
ENVIRONMENT_PARAM = Parameter()
REGION_PARAM = Parameter()
KUBERNETES_VERSION_PARAM = Parameter()
NODE_POOLS_PARAM = Parameter()
DOMAIN_PARAM = Parameter()
ACME_EMAIL_PARAM = Parameter()
GITOPS_REPOSITORY_PARAM = Parameter()
GITOPS_BRANCH_PARAM = Parameter()
GITOPS_TOKEN_PARAM = Parameter()
VAULT_ADDRESS_PARAM = Parameter()
VAULT_ROLE_ID_PARAM = Parameter()
VAULT_SECRET_ID_PARAM = Parameter()
VAULT_CA_CERTIFICATE_PARAM = Parameter()
DIGITALOCEAN_TOKEN_PARAM = Parameter()
SPACES_ACCESS_KEY_PARAM = Parameter()
SPACES_SECRET_KEY_PARAM = Parameter()
CLOUDFLARE_API_TOKEN_SECRET_NAME_PARAM = Parameter()
ENABLE_TRAEFIK_PARAM = Parameter()
ENABLE_CERT_MANAGER_PARAM = Parameter()
ENABLE_EXTERNAL_DNS_PARAM = Parameter()
ENABLE_VAULT_ESO_PARAM = Parameter()
ENABLE_CNPG_PARAM = Parameter()
DRY_RUN_PARAM = Parameter()
RUNNER_TEMP_PARAM = Parameter()
GITHUB_ENV_PARAM = Parameter()


def _build_raw_inputs_from_cli(values: Mapping[str, object]) -> RawInputs:
    """Build raw inputs from CLI overrides."""
    return RawInputs(
        cluster_name=values.get("cluster_name"),
        environment=values.get("environment"),
        region=values.get("region"),
        kubernetes_version=values.get("kubernetes_version"),
        node_pools=values.get("node_pools"),
        domain=values.get("domain"),
        acme_email=values.get("acme_email"),
        gitops_repository=values.get("gitops_repository"),
        gitops_branch=values.get("gitops_branch"),
        gitops_token=values.get("gitops_token"),
        vault_address=values.get("vault_address"),
        vault_role_id=values.get("vault_role_id"),
        vault_secret_id=values.get("vault_secret_id"),
        vault_ca_certificate=values.get("vault_ca_certificate"),
        digitalocean_token=values.get("digitalocean_token"),
        spaces_access_key=values.get("spaces_access_key"),
        spaces_secret_key=values.get("spaces_secret_key"),
        cloudflare_api_token_secret_name=values.get(
            "cloudflare_api_token_secret_name"
        ),
        enable_traefik=values.get("enable_traefik"),
        enable_cert_manager=values.get("enable_cert_manager"),
        enable_external_dns=values.get("enable_external_dns"),
        enable_vault_eso=values.get("enable_vault_eso"),
        enable_cnpg=values.get("enable_cnpg"),
        dry_run=values.get("dry_run"),
        runner_temp=values.get("runner_temp"),
        github_env=values.get("github_env"),
    )


def _run_prepare_flow(values: Mapping[str, object]) -> int:
    """Resolve and export inputs for downstream action steps."""
    raw = _build_raw_inputs_from_cli(values)
    resolved = _resolve_all_inputs(raw)
    prepare_inputs(resolved)
    print("Prepared wildside-infra-k8s inputs.")
    return 0


@app.command()
def main(
    cluster_name: str | None = CLUSTER_NAME_PARAM,
    environment: str | None = ENVIRONMENT_PARAM,
    region: str | None = REGION_PARAM,
    kubernetes_version: str | None = KUBERNETES_VERSION_PARAM,
    node_pools: str | None = NODE_POOLS_PARAM,
    domain: str | None = DOMAIN_PARAM,
    acme_email: str | None = ACME_EMAIL_PARAM,
    gitops_repository: str | None = GITOPS_REPOSITORY_PARAM,
    gitops_branch: str | None = GITOPS_BRANCH_PARAM,
    gitops_token: str | None = GITOPS_TOKEN_PARAM,
    vault_address: str | None = VAULT_ADDRESS_PARAM,
    vault_role_id: str | None = VAULT_ROLE_ID_PARAM,
    vault_secret_id: str | None = VAULT_SECRET_ID_PARAM,
    vault_ca_certificate: str | None = VAULT_CA_CERTIFICATE_PARAM,
    digitalocean_token: str | None = DIGITALOCEAN_TOKEN_PARAM,
    spaces_access_key: str | None = SPACES_ACCESS_KEY_PARAM,
    spaces_secret_key: str | None = SPACES_SECRET_KEY_PARAM,
    cloudflare_api_token_secret_name: str | None = CLOUDFLARE_API_TOKEN_SECRET_NAME_PARAM,
    enable_traefik: str | None = ENABLE_TRAEFIK_PARAM,
    enable_cert_manager: str | None = ENABLE_CERT_MANAGER_PARAM,
    enable_external_dns: str | None = ENABLE_EXTERNAL_DNS_PARAM,
    enable_vault_eso: str | None = ENABLE_VAULT_ESO_PARAM,
    enable_cnpg: str | None = ENABLE_CNPG_PARAM,
    dry_run: str | None = DRY_RUN_PARAM,
    runner_temp: Path | None = RUNNER_TEMP_PARAM,
    github_env: Path | None = GITHUB_ENV_PARAM,
) -> int:
    """Prepare inputs for the wildside-infra-k8s action."""
    values: dict[str, object] = {
        "cluster_name": cluster_name,
        "environment": environment,
        "region": region,
        "kubernetes_version": kubernetes_version,
        "node_pools": node_pools,
        "domain": domain,
        "acme_email": acme_email,
        "gitops_repository": gitops_repository,
        "gitops_branch": gitops_branch,
        "gitops_token": gitops_token,
        "vault_address": vault_address,
        "vault_role_id": vault_role_id,
        "vault_secret_id": vault_secret_id,
        "vault_ca_certificate": vault_ca_certificate,
        "digitalocean_token": digitalocean_token,
        "spaces_access_key": spaces_access_key,
        "spaces_secret_key": spaces_secret_key,
        "cloudflare_api_token_secret_name": cloudflare_api_token_secret_name,
        "enable_traefik": enable_traefik,
        "enable_cert_manager": enable_cert_manager,
        "enable_external_dns": enable_external_dns,
        "enable_vault_eso": enable_vault_eso,
        "enable_cnpg": enable_cnpg,
        "dry_run": dry_run,
        "runner_temp": runner_temp,
        "github_env": github_env,
    }
    return _run_prepare_flow(values)


if __name__ == "__main__":  # pragma: no cover - exercised via CLI
    app()
