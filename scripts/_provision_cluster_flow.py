"""Run OpenTofu for cluster provisioning and export outputs.

This module orchestrates OpenTofu init/plan/apply and exports the resulting
cluster outputs to ``GITHUB_ENV`` for downstream GitHub Actions steps.

Examples
--------
Provision a cluster with prepared inputs:

>>> backend = build_backend_config(inputs)
>>> tfvars = build_tfvars(inputs)
>>> provision_cluster(inputs, backend, tfvars)
"""

from __future__ import annotations

import sys
from pathlib import Path

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
from scripts._provision_cluster_inputs import ProvisionInputs

REPO_ROOT = Path(__file__).resolve().parents[1]
CLUSTER_MODULE_PATH = REPO_ROOT / "infra" / "clusters" / "wildside-infra-k8s"
BACKEND_CONFIG_PATH = REPO_ROOT / "infra" / "backend-config" / "spaces.tfbackend"


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
        outputs_raw = tofu_output(CLUSTER_MODULE_PATH)
    except RuntimeError as exc:
        print(f"error: failed to extract outputs: {exc}", file=sys.stderr)
        return False, {}

    if not isinstance(outputs_raw, dict):
        print("error: tofu output returned unexpected data", file=sys.stderr)
        return False, {}

    return True, outputs_raw


def _extract_output_value(outputs: dict[str, object], key: str) -> str | None:
    """Extract a value from OpenTofu outputs, handling wrapped formats."""
    if key not in outputs:
        return None
    output = outputs[key]
    if output is None:
        return None
    if isinstance(output, dict) and "value" in output:
        output = output["value"]
        if output is None:
            return None
        value = str(output)
        if not value:
            return None
        return value
    value = str(output)
    if not value:
        return None
    return value


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
