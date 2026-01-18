#!/usr/bin/env -S uv run python
# /// script
# requires-python = ">=3.13"
# dependencies = ["cyclopts>=2.9"]
# ///
"""Provision a Kubernetes cluster via OpenTofu.

Configure the OpenTofu backend, run plan/apply for the cluster module, and
export key outputs to the GitHub Actions environment file.

Examples
--------
Run the provisioning workflow with required environment variables:

>>> export CLUSTER_NAME="preview-1"
>>> export ENVIRONMENT="preview"
>>> export REGION="nyc3"
>>> export SPACES_ACCESS_KEY="access-key"
>>> export SPACES_SECRET_KEY="secret-key"
>>> export GITHUB_ENV="/tmp/github-env"
>>> python scripts/provision_cluster.py --region nyc3
"""

from __future__ import annotations

from pathlib import Path

from cyclopts import App, Parameter
from scripts._infra_k8s import mask_secret
from scripts._provision_cluster_flow import export_cluster_outputs, provision_cluster
from scripts._provision_cluster_inputs import (
    RawProvisionInputs,
    build_backend_config,
    build_tfvars,
    resolve_provision_inputs,
)

app = App(help="Provision Kubernetes cluster via OpenTofu.")

CLUSTER_NAME_PARAM = Parameter()
ENVIRONMENT_PARAM = Parameter()
REGION_PARAM = Parameter()
KUBERNETES_VERSION_PARAM = Parameter()
NODE_POOLS_PARAM = Parameter()
SPACES_BUCKET_PARAM = Parameter()
SPACES_REGION_PARAM = Parameter()
SPACES_ACCESS_KEY_PARAM = Parameter()
SPACES_SECRET_KEY_PARAM = Parameter()
RUNNER_TEMP_PARAM = Parameter()
GITHUB_ENV_PARAM = Parameter()
DRY_RUN_PARAM = Parameter()




@app.command()
def main(
    cluster_name: str | None = CLUSTER_NAME_PARAM,
    environment: str | None = ENVIRONMENT_PARAM,
    region: str | None = REGION_PARAM,
    kubernetes_version: str | None = KUBERNETES_VERSION_PARAM,
    node_pools: str | None = NODE_POOLS_PARAM,
    spaces_bucket: str | None = SPACES_BUCKET_PARAM,
    spaces_region: str | None = SPACES_REGION_PARAM,
    spaces_access_key: str | None = SPACES_ACCESS_KEY_PARAM,
    spaces_secret_key: str | None = SPACES_SECRET_KEY_PARAM,
    runner_temp: Path | None = RUNNER_TEMP_PARAM,
    github_env: Path | None = GITHUB_ENV_PARAM,
    dry_run: str | None = DRY_RUN_PARAM,
) -> int:
    """Provision a Kubernetes cluster via OpenTofu.

    Resolve inputs from CLI and environment variables, configure backend state
    for Spaces, and apply the cluster module before exporting outputs to
    ``GITHUB_ENV``.

    Parameters
    ----------
    cluster_name : str | None
        Cluster name override for ``CLUSTER_NAME``.
    environment : str | None
        Environment override for ``ENVIRONMENT``.
    region : str | None
        Region override for ``REGION``.
    kubernetes_version : str | None
        Kubernetes version override for ``KUBERNETES_VERSION``.
    node_pools : str | None
        JSON-encoded node pool configuration override for ``NODE_POOLS``.
    spaces_bucket : str | None
        Spaces bucket override for ``SPACES_BUCKET``.
    spaces_region : str | None
        Spaces region override for ``SPACES_REGION``.
    spaces_access_key : str | None
        Spaces access key override for ``SPACES_ACCESS_KEY``.
    spaces_secret_key : str | None
        Spaces secret key override for ``SPACES_SECRET_KEY``.
    runner_temp : Path | None
        Runner temp directory override for ``RUNNER_TEMP``.
    github_env : Path | None
        Output file override for ``GITHUB_ENV``.
    dry_run : str | None
        Dry-run flag override for ``DRY_RUN``.

    Returns
    -------
    int
        Exit code (0 for success, 1 for failure).

    Examples
    --------
    Run with CLI overrides:

    >>> python scripts/provision_cluster.py --region nyc3 --dry-run true
    """
    # Resolve inputs from environment (CLI args override via resolve_input)
    raw_inputs = RawProvisionInputs(
        cluster_name=cluster_name,
        environment=environment,
        region=region,
        kubernetes_version=kubernetes_version,
        node_pools=node_pools,
        spaces_bucket=spaces_bucket,
        spaces_region=spaces_region,
        spaces_access_key=spaces_access_key,
        spaces_secret_key=spaces_secret_key,
        runner_temp=runner_temp,
        github_env=github_env,
        dry_run=dry_run,
    )
    inputs = resolve_provision_inputs(raw_inputs)

    # Build configurations
    backend_config = build_backend_config(inputs)
    tfvars = build_tfvars(inputs)

    # Mask sensitive values
    mask_secret(inputs.spaces_access_key)
    mask_secret(inputs.spaces_secret_key)

    # Provision cluster
    success, outputs = provision_cluster(inputs, backend_config, tfvars)

    if not success:
        return 1

    # Export outputs to GITHUB_ENV
    if outputs:
        export_cluster_outputs(inputs, outputs)

    print("\nCluster provisioning complete.")
    return 0


if __name__ == "__main__":  # pragma: no cover - CLI entrypoint
    raise SystemExit(app())
