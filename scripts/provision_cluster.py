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

import json
import logging
import sys
from dataclasses import dataclass
from pathlib import Path

from cyclopts import App, Parameter
from scripts._input_resolution import InputResolution, resolve_input
from scripts._infra_k8s import (
    SpacesBackendConfig,
    append_github_env,
    mask_secret,
    parse_bool,
    tofu_apply,
    tofu_init,
    tofu_output,
    tofu_plan,
    write_tfvars,
)

REPO_ROOT = Path(__file__).resolve().parents[1]
CLUSTER_MODULE_PATH = REPO_ROOT / "infra" / "clusters" / "wildside-infra-k8s"
BACKEND_CONFIG_PATH = REPO_ROOT / "infra" / "backend-config" / "spaces.tfbackend"

app = App(help="Provision Kubernetes cluster via OpenTofu.")
logger = logging.getLogger(__name__)


@dataclass(frozen=True, slots=True)
class ProvisionInputs:
    """Inputs for cluster provisioning."""

    # Cluster configuration
    cluster_name: str
    environment: str
    region: str
    kubernetes_version: str | None
    node_pools: str | None

    # Backend configuration
    spaces_bucket: str
    spaces_region: str
    spaces_access_key: str
    spaces_secret_key: str

    # Paths and options
    runner_temp: Path
    github_env: Path
    dry_run: bool


@dataclass(frozen=True, slots=True)
class RawProvisionInputs:
    """Raw provisioning inputs from CLI or defaults."""

    cluster_name: str | None = None
    environment: str | None = None
    region: str | None = None
    kubernetes_version: str | None = None
    node_pools: str | None = None
    spaces_bucket: str | None = None
    spaces_region: str | None = None
    spaces_access_key: str | None = None
    spaces_secret_key: str | None = None
    runner_temp: Path | None = None
    github_env: Path | None = None
    dry_run: str | None = None


def resolve_provision_inputs(
    *,
    cluster_name: str | None = None,
    environment: str | None = None,
    region: str | None = None,
    kubernetes_version: str | None = None,
    node_pools: str | None = None,
    spaces_bucket: str | None = None,
    spaces_region: str | None = None,
    spaces_access_key: str | None = None,
    spaces_secret_key: str | None = None,
    runner_temp: Path | None = None,
    github_env: Path | None = None,
    dry_run: str | None = None,
) -> ProvisionInputs:
    """Resolve provisioning inputs from CLI and environment.

    Normalize CLI values with environment fallbacks so the provisioning
    workflow operates on a consistent set of validated inputs.

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
    ProvisionInputs
        Normalized provisioning inputs ready for use.

    Examples
    --------
    Resolve inputs with a CLI override:

    >>> resolve_provision_inputs(cluster_name="preview-1")
    """
    raw = RawProvisionInputs(
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

    def to_path(value: Path | str) -> Path:
        return value if isinstance(value, Path) else Path(str(value))

    def _resolved(value: str | Path | None, resolution: InputResolution) -> str | Path:
        if value is not None:
            return value
        return resolve_input(None, resolution)

    cluster_name = _resolved(
        raw.cluster_name, InputResolution(env_key="CLUSTER_NAME", required=True)
    )
    environment = _resolved(
        raw.environment, InputResolution(env_key="ENVIRONMENT", required=True)
    )
    region = _resolved(raw.region, InputResolution(env_key="REGION", required=True))
    kubernetes_version = _resolved(
        raw.kubernetes_version, InputResolution(env_key="KUBERNETES_VERSION")
    )
    node_pools = _resolved(raw.node_pools, InputResolution(env_key="NODE_POOLS"))

    # Backend configuration from Spaces
    spaces_bucket = _resolved(
        raw.spaces_bucket,
        InputResolution(env_key="SPACES_BUCKET", default="wildside-tofu-state"),
    )
    spaces_region = _resolved(
        raw.spaces_region, InputResolution(env_key="SPACES_REGION", default="lon1")
    )
    spaces_access_key = _resolved(
        raw.spaces_access_key,
        InputResolution(env_key="SPACES_ACCESS_KEY", required=True),
    )
    spaces_secret_key = _resolved(
        raw.spaces_secret_key,
        InputResolution(env_key="SPACES_SECRET_KEY", required=True),
    )

    # RUNNER_TEMP/GITHUB_ENV/DRY_RUN InputResolution defaults (Path("/tmp"),
    # Path("/tmp/github-env-undefined"), and "false") are intentional
    # local-dev/test fallbacks to avoid hard failures when those env keys are
    # absent; production usage should set the env_key values explicitly.
    runner_temp_raw = _resolved(
        raw.runner_temp,
        InputResolution(env_key="RUNNER_TEMP", default=Path("/tmp"), as_path=True),
    )
    github_env_raw = _resolved(
        raw.github_env,
        InputResolution(
            env_key="GITHUB_ENV",
            default=Path("/tmp/github-env-undefined"),
            as_path=True,
        ),
    )
    dry_run_raw = _resolved(
        raw.dry_run, InputResolution(env_key="DRY_RUN", default="false")
    )

    return ProvisionInputs(
        cluster_name=str(cluster_name),
        environment=str(environment),
        region=str(region),
        kubernetes_version=str(kubernetes_version) if kubernetes_version else None,
        node_pools=str(node_pools) if node_pools else None,
        spaces_bucket=str(spaces_bucket),
        spaces_region=str(spaces_region),
        spaces_access_key=str(spaces_access_key),
        spaces_secret_key=str(spaces_secret_key),
        runner_temp=to_path(runner_temp_raw),
        github_env=to_path(github_env_raw),
        dry_run=parse_bool(str(dry_run_raw) if dry_run_raw else None, default=False),
    )


def build_backend_config(inputs: ProvisionInputs) -> SpacesBackendConfig:
    """Build the OpenTofu backend configuration for Spaces state storage.

    Parameters
    ----------
    inputs : ProvisionInputs
        Normalized provisioning inputs.

    Returns
    -------
    SpacesBackendConfig
        Backend configuration derived from the inputs.
    """
    endpoint = f"https://{inputs.spaces_region}.digitaloceanspaces.com"
    state_key = f"clusters/{inputs.cluster_name}/terraform.tfstate"

    return SpacesBackendConfig(
        bucket=inputs.spaces_bucket,
        region=inputs.spaces_region,
        endpoint=endpoint,
        access_key=inputs.spaces_access_key,
        secret_key=inputs.spaces_secret_key,
        state_key=state_key,
    )


def build_tfvars(inputs: ProvisionInputs) -> dict[str, object]:
    """Build OpenTofu variables for cluster provisioning.

    Parameters
    ----------
    inputs : ProvisionInputs
        Normalized provisioning inputs.

    Returns
    -------
    dict[str, object]
        Mapping of OpenTofu variables to render into tfvars.
    """
    variables: dict[str, object] = {
        "cluster_name": inputs.cluster_name,
        "environment": inputs.environment,
        "region": inputs.region,
    }

    if inputs.kubernetes_version:
        variables["kubernetes_version"] = inputs.kubernetes_version

    if inputs.node_pools:
        try:
            node_pools = json.loads(inputs.node_pools)
        except json.JSONDecodeError:
            logger.warning("Invalid node_pools JSON ignored: %s", inputs.node_pools)
        else:
            variables["node_pools"] = node_pools

    return variables


def provision_cluster(
    inputs: ProvisionInputs,
    backend_config: SpacesBackendConfig,
    tfvars: dict[str, object],
) -> tuple[bool, dict[str, object]]:
    """Run OpenTofu init, plan, and apply for cluster provisioning.

    Parameters
    ----------
    inputs : ProvisionInputs
        Normalized provisioning inputs.
    backend_config : SpacesBackendConfig
        Backend configuration for state storage.
    tfvars : dict[str, object]
        OpenTofu variables for the cluster module.

    Returns
    -------
    tuple[bool, dict[str, object]]
        Success flag and outputs from ``tofu output`` if successful.

    Examples
    --------
    Provision using a prepared backend configuration:

    >>> backend = build_backend_config(inputs)
    >>> tfvars = build_tfvars(inputs)
    >>> provision_cluster(inputs, backend, tfvars)
    """
    work_dir = inputs.runner_temp / "provision-cluster"
    work_dir.mkdir(parents=True, exist_ok=True)

    # Write tfvars to temp file
    var_file = work_dir / "cluster.tfvars.json"
    write_tfvars(var_file, tfvars)

    print(f"Provisioning cluster '{inputs.cluster_name}' in {inputs.region}...")
    print(f"  Environment: {inputs.environment}")
    print(f"  State key: {backend_config.state_key}")
    print(f"  Dry run: {inputs.dry_run}")

    # Initialise with backend configuration
    print("\n--- Running tofu init ---")
    init_result = tofu_init(
        CLUSTER_MODULE_PATH,
        backend_config,
        BACKEND_CONFIG_PATH,
    )
    if not init_result.success:
        print(f"error: tofu init failed: {init_result.stderr}", file=sys.stderr)
        return False, {}

    print(init_result.stdout)

    # Run plan
    print("\n--- Running tofu plan ---")
    plan_result = tofu_plan(CLUSTER_MODULE_PATH, var_file)
    if not plan_result.success:
        print(f"error: tofu plan failed: {plan_result.stderr}", file=sys.stderr)
        return False, {}

    print(plan_result.stdout)

    # In dry-run mode, stop after plan
    if inputs.dry_run:
        print("\nDry run mode - skipping apply")
        return True, {}

    # Run apply
    print("\n--- Running tofu apply ---")
    apply_result = tofu_apply(CLUSTER_MODULE_PATH, var_file, auto_approve=True)
    if not apply_result.success:
        print(f"error: tofu apply failed: {apply_result.stderr}", file=sys.stderr)
        return False, {}

    print(apply_result.stdout)

    # Extract outputs
    print("\n--- Extracting outputs ---")
    try:
        outputs = tofu_output(CLUSTER_MODULE_PATH)
    except RuntimeError as exc:
        print(f"error: failed to extract outputs: {exc}", file=sys.stderr)
        return False, {}

    return True, outputs


def _extract_output_value(outputs: dict[str, object], key: str) -> str | None:
    """Extract a value from OpenTofu outputs, handling both direct and wrapped formats.

    OpenTofu outputs may be returned as direct values or as dicts with a "value" key.
    This helper normalises both formats to a string, or returns None if the key is absent.
    """
    if key not in outputs:
        return None
    output = outputs[key]
    if isinstance(output, dict) and "value" in output:
        return str(output["value"])
    return str(output)


def export_cluster_outputs(
    inputs: ProvisionInputs,
    outputs: dict[str, object],
) -> None:
    """Export cluster outputs to the GitHub Actions environment file.

    Parameters
    ----------
    inputs : ProvisionInputs
        Normalized provisioning inputs.
    outputs : dict[str, object]
        OpenTofu outputs for the cluster module.

    Returns
    -------
    None
        This function writes to ``GITHUB_ENV`` and returns nothing.
    """
    env_vars: dict[str, str] = {}

    if cluster_id := _extract_output_value(outputs, "cluster_id"):
        env_vars["CLUSTER_ID"] = cluster_id

    if endpoint := _extract_output_value(outputs, "endpoint"):
        env_vars["CLUSTER_ENDPOINT"] = endpoint

    if kubeconfig := _extract_output_value(outputs, "kubeconfig"):
        mask_secret(kubeconfig)
        env_vars["KUBECONFIG_RAW"] = kubeconfig

    if env_vars:
        append_github_env(inputs.github_env, env_vars)
        print(f"Exported {len(env_vars)} variables to GITHUB_ENV")


@app.command()
def main(
    cluster_name: str | None = Parameter(),
    environment: str | None = Parameter(),
    region: str | None = Parameter(),
    kubernetes_version: str | None = Parameter(),
    node_pools: str | None = Parameter(),
    spaces_bucket: str | None = Parameter(),
    spaces_region: str | None = Parameter(),
    spaces_access_key: str | None = Parameter(),
    spaces_secret_key: str | None = Parameter(),
    runner_temp: Path | None = Parameter(),
    github_env: Path | None = Parameter(),
    dry_run: str | None = Parameter(),
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
    inputs = resolve_provision_inputs(
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
