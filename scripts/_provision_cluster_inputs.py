"""Resolve inputs and build tfvars for cluster provisioning."""

from __future__ import annotations

import json
import logging
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path

from scripts._input_resolution import InputResolution, resolve_input
from scripts._infra_k8s import SpacesBackendConfig, parse_bool

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


def _resolve_cluster_inputs(
    raw: RawProvisionInputs,
    _resolved: Callable[[str | Path | None, InputResolution], str | Path | None],
) -> tuple[str, str, str, str | None, str | None]:
    """Resolve cluster configuration inputs."""
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
    return (
        str(cluster_name),
        str(environment),
        str(region),
        str(kubernetes_version) if kubernetes_version else None,
        str(node_pools) if node_pools else None,
    )


def _resolve_spaces_config(
    raw: RawProvisionInputs,
    _resolved: Callable[[str | Path | None, InputResolution], str | Path | None],
) -> tuple[str, str, str, str]:
    """Resolve Spaces backend configuration inputs."""
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
    return (
        str(spaces_bucket),
        str(spaces_region),
        str(spaces_access_key),
        str(spaces_secret_key),
    )


def _resolve_execution_config(
    raw: RawProvisionInputs,
    _resolved: Callable[[str | Path | None, InputResolution], str | Path | None],
    to_path: Callable[[Path | str], Path],
) -> tuple[Path, Path, bool]:
    """Resolve execution configuration inputs."""
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
    return (
        to_path(runner_temp_raw),
        to_path(github_env_raw),
        parse_bool(str(dry_run_raw) if dry_run_raw else None, default=False),
    )


def resolve_provision_inputs(raw: RawProvisionInputs) -> ProvisionInputs:
    """Resolve provisioning inputs from CLI and environment.

    Normalize CLI values with environment fallbacks so the provisioning
    workflow operates on a consistent set of validated inputs.

    Parameters
    ----------
    raw : RawProvisionInputs
        Raw provisioning inputs sourced from CLI arguments or defaults.

    Returns
    -------
    ProvisionInputs
        Normalized provisioning inputs ready for use.

    Examples
    --------
    Resolve inputs with a CLI override:

    >>> resolve_provision_inputs(RawProvisionInputs(cluster_name="preview-1"))
    """

    def to_path(value: Path | str | None) -> Path:
        return value if isinstance(value, Path) else Path(str(value))

    def _resolved(
        value: str | Path | None,
        resolution: InputResolution,
    ) -> str | Path | None:
        if value is not None:
            return value
        return resolve_input(None, resolution)

    (
        cluster_name,
        environment,
        region,
        kubernetes_version,
        node_pools,
    ) = _resolve_cluster_inputs(raw, _resolved)
    (
        spaces_bucket,
        spaces_region,
        spaces_access_key,
        spaces_secret_key,
    ) = _resolve_spaces_config(raw, _resolved)
    runner_temp, github_env, dry_run = _resolve_execution_config(raw, _resolved, to_path)

    return ProvisionInputs(
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
