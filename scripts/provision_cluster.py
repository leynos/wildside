#!/usr/bin/env -S uv run python
# /// script
# requires-python = ">=3.13"
# dependencies = ["cyclopts>=2.9"]
# ///
"""Provision Kubernetes cluster via OpenTofu.

This script:
- configures the OpenTofu backend with Spaces credentials;
- runs tofu init, plan, and apply for cluster provisioning;
- extracts cluster outputs (ID, endpoint, kubeconfig); and
- exports cluster metadata to $GITHUB_ENV.
"""

from __future__ import annotations

import sys
from dataclasses import dataclass
from pathlib import Path

from cyclopts import App, Parameter
from scripts._input_resolution import InputResolution, resolve_input
from scripts._infra_k8s import (
    SpacesBackendConfig,
    append_github_env,
    mask_secret,
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


def _parse_bool(value: str | None, default: bool = False) -> bool:
    """Parse a boolean string value."""
    if value is None:
        return default
    return value.lower() in ("true", "1", "yes")


def resolve_provision_inputs() -> ProvisionInputs:
    """Resolve provisioning inputs from environment."""
    cluster_name = resolve_input(
        None, InputResolution(env_key="CLUSTER_NAME", required=True)
    )
    environment = resolve_input(
        None, InputResolution(env_key="ENVIRONMENT", required=True)
    )
    region = resolve_input(None, InputResolution(env_key="REGION", required=True))
    kubernetes_version = resolve_input(
        None, InputResolution(env_key="KUBERNETES_VERSION")
    )
    node_pools = resolve_input(None, InputResolution(env_key="NODE_POOLS"))

    # Backend configuration from Spaces
    spaces_bucket = resolve_input(
        None,
        InputResolution(env_key="SPACES_BUCKET", default="wildside-tofu-state"),
    )
    spaces_region = resolve_input(
        None, InputResolution(env_key="SPACES_REGION", default="lon1")
    )
    spaces_access_key = resolve_input(
        None, InputResolution(env_key="SPACES_ACCESS_KEY", required=True)
    )
    spaces_secret_key = resolve_input(
        None, InputResolution(env_key="SPACES_SECRET_KEY", required=True)
    )

    runner_temp_raw = resolve_input(
        None,
        InputResolution(env_key="RUNNER_TEMP", default=Path("/tmp"), as_path=True),
    )
    github_env_raw = resolve_input(
        None,
        InputResolution(
            env_key="GITHUB_ENV",
            default=Path("/tmp/github-env-undefined"),
            as_path=True,
        ),
    )
    dry_run_raw = resolve_input(
        None, InputResolution(env_key="DRY_RUN", default="false")
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
        runner_temp=(
            runner_temp_raw
            if isinstance(runner_temp_raw, Path)
            else Path(str(runner_temp_raw))
        ),
        github_env=(
            github_env_raw
            if isinstance(github_env_raw, Path)
            else Path(str(github_env_raw))
        ),
        dry_run=_parse_bool(str(dry_run_raw) if dry_run_raw else None),
    )


def build_backend_config(inputs: ProvisionInputs) -> SpacesBackendConfig:
    """Build backend configuration for Spaces state storage."""
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
    """Build tfvars for cluster provisioning."""
    variables: dict[str, object] = {
        "cluster_name": inputs.cluster_name,
        "environment": inputs.environment,
        "region": inputs.region,
    }

    if inputs.kubernetes_version:
        variables["kubernetes_version"] = inputs.kubernetes_version

    if inputs.node_pools:
        # Node pools should be passed as JSON, parse and include
        import json

        try:
            variables["node_pools"] = json.loads(inputs.node_pools)
        except json.JSONDecodeError:
            print(f"warning: invalid node_pools JSON, ignoring: {inputs.node_pools}")

    return variables


def provision_cluster(
    inputs: ProvisionInputs,
    backend_config: SpacesBackendConfig,
    tfvars: dict[str, object],
) -> tuple[bool, dict[str, object]]:
    """Run OpenTofu init, plan, and apply for cluster provisioning.

    Returns a tuple of (success, outputs).
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
    """Export cluster outputs to GITHUB_ENV."""
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
    """Provision Kubernetes cluster via OpenTofu.

    This command resolves inputs from environment variables (set by
    prepare_infra_k8s_inputs.py), configures the OpenTofu backend, runs
    init/plan/apply, and exports cluster outputs to GITHUB_ENV.
    """
    # Resolve inputs from environment (CLI args override)
    inputs = resolve_provision_inputs()

    # Override with CLI args if provided
    if cluster_name is not None:
        inputs = ProvisionInputs(
            cluster_name=cluster_name,
            environment=inputs.environment,
            region=inputs.region,
            kubernetes_version=inputs.kubernetes_version,
            node_pools=inputs.node_pools,
            spaces_bucket=inputs.spaces_bucket,
            spaces_region=inputs.spaces_region,
            spaces_access_key=inputs.spaces_access_key,
            spaces_secret_key=inputs.spaces_secret_key,
            runner_temp=inputs.runner_temp,
            github_env=inputs.github_env,
            dry_run=inputs.dry_run,
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
