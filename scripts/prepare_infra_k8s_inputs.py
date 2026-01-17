#!/usr/bin/env -S uv run python
# /// script
# requires-python = ">=3.13"
# dependencies = ["cyclopts>=2.9"]
# ///
"""Prepare inputs for the wildside-infra-k8s GitHub Action.

This script:
- resolves and validates action inputs from environment variables;
- masks sensitive values in logs; and
- exports resolved values to $GITHUB_ENV.
"""

from __future__ import annotations

import json
import tempfile
from dataclasses import dataclass
from pathlib import Path
from collections.abc import Callable

from cyclopts import App, Parameter
from scripts._input_resolution import InputResolution, resolve_input
from scripts._infra_k8s import (
    append_github_env,
    mask_secret,
    parse_bool,
    parse_node_pools,
    validate_cluster_name,
)

type Mask = Callable[[str], None]

app = App(help="Prepare wildside-infra-k8s action inputs.")


@dataclass(frozen=True, slots=True)
class RawInputs:
    """Raw CLI and environment inputs before resolution."""

    cluster_name: str | None
    environment: str | None
    region: str | None
    kubernetes_version: str | None
    node_pools: str | None
    domain: str | None
    acme_email: str | None
    gitops_repository: str | None
    gitops_branch: str | None
    gitops_token: str | None
    vault_address: str | None
    vault_role_id: str | None
    vault_secret_id: str | None
    vault_ca_certificate: str | None
    digitalocean_token: str | None
    spaces_access_key: str | None
    spaces_secret_key: str | None
    cloudflare_api_token_secret_name: str | None
    enable_traefik: str | None
    enable_cert_manager: str | None
    enable_external_dns: str | None
    enable_vault_eso: str | None
    enable_cnpg: str | None
    dry_run: str | None
    runner_temp: Path | None
    github_env: Path | None


@dataclass(frozen=True, slots=True)
class ResolvedInputs:
    """All CLI and environment inputs resolved to their final values."""

    cluster_name: str
    environment: str
    region: str
    kubernetes_version: str | None
    node_pools: list[dict[str, object]] | None
    domain: str
    acme_email: str
    gitops_repository: str
    gitops_branch: str
    gitops_token: str
    vault_address: str
    vault_role_id: str
    vault_secret_id: str
    vault_ca_certificate: str | None
    digitalocean_token: str
    spaces_access_key: str
    spaces_secret_key: str
    cloudflare_api_token_secret_name: str
    enable_traefik: bool
    enable_cert_manager: bool
    enable_external_dns: bool
    enable_vault_eso: bool
    enable_cnpg: bool
    dry_run: bool
    runner_temp: Path
    github_env: Path


def _resolve_cluster_config(
    raw: RawInputs,
) -> tuple[str, str, str, str | None, str | None]:
    """Resolve cluster configuration inputs."""
    cluster_name = resolve_input(
        raw.cluster_name,
        InputResolution(env_key="INPUT_CLUSTER_NAME", required=True),
    )
    environment = resolve_input(
        raw.environment,
        InputResolution(env_key="INPUT_ENVIRONMENT", required=True),
    )
    region = resolve_input(
        raw.region,
        InputResolution(env_key="INPUT_REGION", required=True),
    )
    kubernetes_version = resolve_input(
        raw.kubernetes_version,
        InputResolution(env_key="INPUT_KUBERNETES_VERSION"),
    )
    node_pools_raw = resolve_input(
        raw.node_pools,
        InputResolution(env_key="INPUT_NODE_POOLS"),
    )
    return (
        cluster_name,
        environment,
        region,
        kubernetes_version,
        node_pools_raw,
    )


def _resolve_domain_config(raw: RawInputs) -> tuple[str, str]:
    """Resolve domain configuration inputs."""
    domain = resolve_input(
        raw.domain,
        InputResolution(env_key="INPUT_DOMAIN", required=True),
    )
    acme_email = resolve_input(
        raw.acme_email,
        InputResolution(env_key="INPUT_ACME_EMAIL", required=True),
    )
    return domain, acme_email


def _resolve_gitops_config(raw: RawInputs) -> tuple[str, str, str]:
    """Resolve GitOps configuration inputs."""
    gitops_repository = resolve_input(
        raw.gitops_repository,
        InputResolution(env_key="INPUT_GITOPS_REPOSITORY", required=True),
    )
    gitops_branch = resolve_input(
        raw.gitops_branch,
        InputResolution(env_key="INPUT_GITOPS_BRANCH", default="main"),
    )
    gitops_token = resolve_input(
        raw.gitops_token,
        InputResolution(env_key="INPUT_GITOPS_TOKEN", required=True),
    )
    return gitops_repository, gitops_branch, gitops_token


def _resolve_vault_config(raw: RawInputs) -> tuple[str, str, str, str | None]:
    """Resolve vault configuration inputs."""
    vault_address = resolve_input(
        raw.vault_address,
        InputResolution(env_key="INPUT_VAULT_ADDRESS", required=True),
    )
    vault_role_id = resolve_input(
        raw.vault_role_id,
        InputResolution(env_key="INPUT_VAULT_ROLE_ID", required=True),
    )
    vault_secret_id = resolve_input(
        raw.vault_secret_id,
        InputResolution(env_key="INPUT_VAULT_SECRET_ID", required=True),
    )
    vault_ca_certificate = resolve_input(
        raw.vault_ca_certificate,
        InputResolution(env_key="INPUT_VAULT_CA_CERTIFICATE"),
    )
    return vault_address, vault_role_id, vault_secret_id, vault_ca_certificate


def _resolve_cloud_credentials(raw: RawInputs) -> tuple[str, str, str]:
    """Resolve cloud credential inputs."""
    digitalocean_token = resolve_input(
        raw.digitalocean_token,
        InputResolution(env_key="INPUT_DIGITALOCEAN_TOKEN", required=True),
    )
    spaces_access_key = resolve_input(
        raw.spaces_access_key,
        InputResolution(env_key="INPUT_SPACES_ACCESS_KEY", required=True),
    )
    spaces_secret_key = resolve_input(
        raw.spaces_secret_key,
        InputResolution(env_key="INPUT_SPACES_SECRET_KEY", required=True),
    )
    return digitalocean_token, spaces_access_key, spaces_secret_key


def _resolve_feature_flags(
    raw: RawInputs,
) -> tuple[str, str, str, str, str, str]:
    """Resolve feature flag inputs."""
    cloudflare_api_token_secret_name = resolve_input(
        raw.cloudflare_api_token_secret_name,
        InputResolution(
            env_key="INPUT_CLOUDFLARE_API_TOKEN_SECRET_NAME",
            default="cloudflare-api-token",
        ),
    )
    enable_traefik = resolve_input(
        raw.enable_traefik,
        InputResolution(env_key="INPUT_ENABLE_TRAEFIK", default="true"),
    )
    enable_cert_manager = resolve_input(
        raw.enable_cert_manager,
        InputResolution(env_key="INPUT_ENABLE_CERT_MANAGER", default="true"),
    )
    enable_external_dns = resolve_input(
        raw.enable_external_dns,
        InputResolution(env_key="INPUT_ENABLE_EXTERNAL_DNS", default="true"),
    )
    enable_vault_eso = resolve_input(
        raw.enable_vault_eso,
        InputResolution(env_key="INPUT_ENABLE_VAULT_ESO", default="true"),
    )
    enable_cnpg = resolve_input(
        raw.enable_cnpg,
        InputResolution(env_key="INPUT_ENABLE_CNPG", default="true"),
    )
    return (
        cloudflare_api_token_secret_name,
        enable_traefik,
        enable_cert_manager,
        enable_external_dns,
        enable_vault_eso,
        enable_cnpg,
    )


def _resolve_execution_config(raw: RawInputs) -> tuple[str, Path, Path]:
    """Resolve execution configuration inputs."""
    dry_run = resolve_input(
        raw.dry_run,
        InputResolution(env_key="INPUT_DRY_RUN", default="false"),
    )
    runner_temp = resolve_input(
        raw.runner_temp,
        InputResolution(
            env_key="RUNNER_TEMP",
            default=Path(tempfile.gettempdir()),
            as_path=True,
        ),
    )
    github_env = resolve_input(
        raw.github_env,
        InputResolution(
            env_key="GITHUB_ENV",
            default=Path(tempfile.gettempdir()) / "github-env-undefined",
            as_path=True,
        ),
    )
    return dry_run, runner_temp, github_env


def _resolve_all_inputs(raw: RawInputs) -> ResolvedInputs:
    """Resolve CLI and environment inputs to their canonical types."""
    (
        cluster_name,
        environment,
        region,
        kubernetes_version,
        node_pools_raw,
    ) = _resolve_cluster_config(raw)
    domain, acme_email = _resolve_domain_config(raw)
    gitops_repository, gitops_branch, gitops_token = _resolve_gitops_config(raw)
    (
        vault_address,
        vault_role_id,
        vault_secret_id,
        vault_ca_certificate,
    ) = _resolve_vault_config(raw)
    (
        digitalocean_token,
        spaces_access_key,
        spaces_secret_key,
    ) = _resolve_cloud_credentials(raw)
    (
        cloudflare_api_token_secret_name,
        enable_traefik,
        enable_cert_manager,
        enable_external_dns,
        enable_vault_eso,
        enable_cnpg,
    ) = _resolve_feature_flags(raw)
    dry_run, runner_temp, github_env = _resolve_execution_config(raw)

    # Validate cluster name format
    validated_cluster_name = validate_cluster_name(str(cluster_name))

    return ResolvedInputs(
        cluster_name=validated_cluster_name,
        environment=str(environment),
        region=str(region),
        kubernetes_version=str(kubernetes_version) if kubernetes_version else None,
        node_pools=parse_node_pools(str(node_pools_raw) if node_pools_raw else None),
        domain=str(domain),
        acme_email=str(acme_email),
        gitops_repository=str(gitops_repository),
        gitops_branch=str(gitops_branch) if gitops_branch else "main",
        gitops_token=str(gitops_token),
        vault_address=str(vault_address),
        vault_role_id=str(vault_role_id),
        vault_secret_id=str(vault_secret_id),
        vault_ca_certificate=str(vault_ca_certificate) if vault_ca_certificate else None,
        digitalocean_token=str(digitalocean_token),
        spaces_access_key=str(spaces_access_key),
        spaces_secret_key=str(spaces_secret_key),
        cloudflare_api_token_secret_name=str(cloudflare_api_token_secret_name)
        if cloudflare_api_token_secret_name
        else "cloudflare-api-token",
        enable_traefik=parse_bool(str(enable_traefik) if enable_traefik else None),
        enable_cert_manager=parse_bool(
            str(enable_cert_manager) if enable_cert_manager else None
        ),
        enable_external_dns=parse_bool(
            str(enable_external_dns) if enable_external_dns else None
        ),
        enable_vault_eso=parse_bool(
            str(enable_vault_eso) if enable_vault_eso else None
        ),
        enable_cnpg=parse_bool(str(enable_cnpg) if enable_cnpg else None),
        dry_run=parse_bool(str(dry_run) if dry_run else None, default=False),
        runner_temp=runner_temp if isinstance(runner_temp, Path) else Path(str(runner_temp)),
        github_env=github_env if isinstance(github_env, Path) else Path(str(github_env)),
    )


def prepare_inputs(inputs: ResolvedInputs, mask: Mask = print) -> None:
    """Mask secrets and export environment variables.

    Parameters
    ----------
    inputs : ResolvedInputs
        Resolved input values.
    mask : Mask
        Function to emit masking commands (default: print).
    """

    # Mask sensitive values
    mask_secret(inputs.gitops_token, mask)
    mask_secret(inputs.vault_role_id, mask)
    mask_secret(inputs.vault_secret_id, mask)
    mask_secret(inputs.digitalocean_token, mask)
    mask_secret(inputs.spaces_access_key, mask)
    mask_secret(inputs.spaces_secret_key, mask)
    if inputs.vault_ca_certificate:
        mask_secret(inputs.vault_ca_certificate, mask)

    # Export to GITHUB_ENV
    env_vars = {
        "CLUSTER_NAME": inputs.cluster_name,
        "ENVIRONMENT": inputs.environment,
        "REGION": inputs.region,
        "DOMAIN": inputs.domain,
        "ACME_EMAIL": inputs.acme_email,
        "GITOPS_REPOSITORY": inputs.gitops_repository,
        "GITOPS_BRANCH": inputs.gitops_branch,
        "VAULT_ADDRESS": inputs.vault_address,
        "CLOUDFLARE_API_TOKEN_SECRET_NAME": inputs.cloudflare_api_token_secret_name,
        "ENABLE_TRAEFIK": str(inputs.enable_traefik).lower(),
        "ENABLE_CERT_MANAGER": str(inputs.enable_cert_manager).lower(),
        "ENABLE_EXTERNAL_DNS": str(inputs.enable_external_dns).lower(),
        "ENABLE_VAULT_ESO": str(inputs.enable_vault_eso).lower(),
        "ENABLE_CNPG": str(inputs.enable_cnpg).lower(),
        "DRY_RUN": str(inputs.dry_run).lower(),
        # Sensitive values are exported but masked
        "DIGITALOCEAN_TOKEN": inputs.digitalocean_token,
        "SPACES_ACCESS_KEY": inputs.spaces_access_key,
        "SPACES_SECRET_KEY": inputs.spaces_secret_key,
        "VAULT_ROLE_ID": inputs.vault_role_id,
        "VAULT_SECRET_ID": inputs.vault_secret_id,
        "GITOPS_TOKEN": inputs.gitops_token,
    }

    if inputs.kubernetes_version:
        env_vars["KUBERNETES_VERSION"] = inputs.kubernetes_version
    if inputs.node_pools is not None:
        env_vars["NODE_POOLS"] = json.dumps(inputs.node_pools)
    if inputs.vault_ca_certificate:
        env_vars["VAULT_CA_CERTIFICATE"] = inputs.vault_ca_certificate

    append_github_env(inputs.github_env, env_vars)


@app.command()
def main(
    cluster_name: str | None = Parameter(),
    environment: str | None = Parameter(),
    region: str | None = Parameter(),
    kubernetes_version: str | None = Parameter(),
    node_pools: str | None = Parameter(),
    domain: str | None = Parameter(),
    acme_email: str | None = Parameter(),
    gitops_repository: str | None = Parameter(),
    gitops_branch: str | None = Parameter(),
    gitops_token: str | None = Parameter(),
    vault_address: str | None = Parameter(),
    vault_role_id: str | None = Parameter(),
    vault_secret_id: str | None = Parameter(),
    vault_ca_certificate: str | None = Parameter(),
    digitalocean_token: str | None = Parameter(),
    spaces_access_key: str | None = Parameter(),
    spaces_secret_key: str | None = Parameter(),
    cloudflare_api_token_secret_name: str | None = Parameter(),
    enable_traefik: str | None = Parameter(),
    enable_cert_manager: str | None = Parameter(),
    enable_external_dns: str | None = Parameter(),
    enable_vault_eso: str | None = Parameter(),
    enable_cnpg: str | None = Parameter(),
    dry_run: str | None = Parameter(),
    runner_temp: Path | None = Parameter(),
    github_env: Path | None = Parameter(),
) -> None:
    """Prepare inputs for the wildside-infra-k8s action.

    This command resolves inputs from CLI arguments and environment variables,
    validates them, masks secrets, and exports resolved values to GITHUB_ENV.
    """

    raw = RawInputs(
        cluster_name=cluster_name,
        environment=environment,
        region=region,
        kubernetes_version=kubernetes_version,
        node_pools=node_pools,
        domain=domain,
        acme_email=acme_email,
        gitops_repository=gitops_repository,
        gitops_branch=gitops_branch,
        gitops_token=gitops_token,
        vault_address=vault_address,
        vault_role_id=vault_role_id,
        vault_secret_id=vault_secret_id,
        vault_ca_certificate=vault_ca_certificate,
        digitalocean_token=digitalocean_token,
        spaces_access_key=spaces_access_key,
        spaces_secret_key=spaces_secret_key,
        cloudflare_api_token_secret_name=cloudflare_api_token_secret_name,
        enable_traefik=enable_traefik,
        enable_cert_manager=enable_cert_manager,
        enable_external_dns=enable_external_dns,
        enable_vault_eso=enable_vault_eso,
        enable_cnpg=enable_cnpg,
        dry_run=dry_run,
        runner_temp=runner_temp,
        github_env=github_env,
    )

    inputs = _resolve_all_inputs(raw)
    prepare_inputs(inputs)


if __name__ == "__main__":  # pragma: no cover - exercised via CLI
    app()
