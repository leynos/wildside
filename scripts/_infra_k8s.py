"""Shared utilities for the wildside-infra-k8s action.

This module provides helpers for OpenTofu invocation, backend configuration,
manifest handling, and GitHub Action context management.
"""

from __future__ import annotations

import json
import os
import subprocess
from dataclasses import dataclass, field
from pathlib import Path
from collections import abc as cabc


@dataclass(frozen=True, slots=True)
class SpacesBackendConfig:
    """Configuration for DigitalOcean Spaces state backend."""

    bucket: str
    region: str
    endpoint: str
    access_key: str
    secret_key: str
    state_key: str


@dataclass(frozen=True, slots=True)
class ClusterConfig:
    """Configuration for a Kubernetes cluster."""

    name: str
    region: str
    environment: str
    kubernetes_version: str | None = None
    node_pools: list[dict[str, object]] | None = None
    tags: list[str] = field(default_factory=list)


@dataclass(frozen=True, slots=True)
class PlatformConfig:
    """Configuration for platform module rendering."""

    cluster_name: str
    domain: str
    acme_email: str
    cloudflare_api_token_secret_name: str
    enable_traefik: bool = True
    enable_cert_manager: bool = True
    enable_external_dns: bool = True
    enable_vault_eso: bool = True
    enable_cnpg: bool = True
    vault_address: str | None = None
    vault_ca_bundle_pem: str | None = None
    vault_approle_role_id: str | None = None
    vault_approle_secret_id: str | None = None


@dataclass(frozen=True, slots=True)
class GitOpsConfig:
    """Configuration for GitOps repository operations."""

    repository: str
    branch: str
    token: str
    base_path: str = "."


@dataclass(frozen=True, slots=True)
class TofuResult:
    """Result of an OpenTofu command execution."""

    success: bool
    stdout: str
    stderr: str
    return_code: int


def mask_secret(value: str, stream: cabc.Callable[[str], None] = print) -> None:
    """Emit GitHub Action secret masking command."""
    if value:
        stream(f"::add-mask::{value}")


def append_github_env(env_file: Path, variables: dict[str, str]) -> None:
    """Append environment variables to GITHUB_ENV file."""
    env_file.parent.mkdir(parents=True, exist_ok=True)
    with env_file.open("a", encoding="utf-8") as handle:
        for key, value in variables.items():
            handle.write(f"{key}={value}\n")


def append_github_output(output_file: Path, outputs: dict[str, str]) -> None:
    """Append outputs to GITHUB_OUTPUT file."""
    output_file.parent.mkdir(parents=True, exist_ok=True)
    with output_file.open("a", encoding="utf-8") as handle:
        for key, value in outputs.items():
            handle.write(f"{key}={value}\n")


def run_tofu(
    args: list[str],
    cwd: Path,
    env: cabc.Mapping[str, str] | None = None,
    capture_output: bool = True,
) -> TofuResult:
    """Execute an OpenTofu command and return the result.

    Parameters
    ----------
    args : list[str]
        Command arguments (without 'tofu' prefix).
    cwd : Path
        Working directory for the command.
    env : Mapping[str, str] | None, optional
        Environment variables to set for the command.
    capture_output : bool, optional
        Whether to capture stdout/stderr (default: True).

    Returns
    -------
    TofuResult
        Result containing success status, output, and return code.
    """
    cmd = ["tofu", *args]
    merged_env = {**os.environ, **(env or {})}

    result = subprocess.run(
        cmd,
        cwd=cwd,
        env=merged_env,
        capture_output=capture_output,
        text=True,
        check=False,
    )

    return TofuResult(
        success=result.returncode == 0,
        stdout=result.stdout if capture_output else "",
        stderr=result.stderr if capture_output else "",
        return_code=result.returncode,
    )


def tofu_init(
    cwd: Path,
    backend_config: SpacesBackendConfig,
    backend_config_file: Path,
) -> TofuResult:
    """Initialise OpenTofu with backend configuration.

    Parameters
    ----------
    cwd : Path
        OpenTofu configuration directory.
    backend_config : SpacesBackendConfig
        Backend configuration for state storage.
    backend_config_file : Path
        Path to the .tfbackend file.

    Returns
    -------
    TofuResult
        Result of the init command.
    """
    args = [
        "init",
        f"-backend-config={backend_config_file}",
        f"-backend-config=access_key={backend_config.access_key}",
        f"-backend-config=secret_key={backend_config.secret_key}",
        f"-backend-config=key={backend_config.state_key}",
        "-input=false",
    ]
    return run_tofu(args, cwd)


def tofu_plan(cwd: Path, var_file: Path | None = None) -> TofuResult:
    """Run OpenTofu plan.

    Parameters
    ----------
    cwd : Path
        OpenTofu configuration directory.
    var_file : Path | None, optional
        Path to a tfvars file.

    Returns
    -------
    TofuResult
        Result of the plan command.
    """
    args = ["plan", "-input=false"]
    if var_file:
        args.append(f"-var-file={var_file}")
    return run_tofu(args, cwd)


def tofu_apply(
    cwd: Path,
    var_file: Path | None = None,
    auto_approve: bool = True,
) -> TofuResult:
    """Run OpenTofu apply.

    Parameters
    ----------
    cwd : Path
        OpenTofu configuration directory.
    var_file : Path | None, optional
        Path to a tfvars file.
    auto_approve : bool, optional
        Whether to auto-approve (default: True).

    Returns
    -------
    TofuResult
        Result of the apply command.
    """
    args = ["apply", "-input=false"]
    if auto_approve:
        args.append("-auto-approve")
    if var_file:
        args.append(f"-var-file={var_file}")
    return run_tofu(args, cwd)


def tofu_output(cwd: Path, name: str | None = None) -> dict[str, object]:
    """Retrieve OpenTofu outputs as JSON.

    Parameters
    ----------
    cwd : Path
        OpenTofu configuration directory.
    name : str | None, optional
        Specific output name to retrieve.

    Returns
    -------
    dict[str, object]
        Parsed JSON output. If name is specified, returns the value directly.
    """
    args = ["output", "-json"]
    if name:
        args.append(name)
    result = run_tofu(args, cwd)

    if not result.success:
        msg = f"tofu output failed: {result.stderr}"
        raise RuntimeError(msg)

    return json.loads(result.stdout)


def write_tfvars(path: Path, variables: dict[str, object]) -> None:
    """Write variables to a tfvars.json file.

    Parameters
    ----------
    path : Path
        Destination path for the tfvars file.
    variables : dict[str, object]
        Variables to write.
    """
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(variables, indent=2), encoding="utf-8")


def write_manifests(output_dir: Path, manifests: dict[str, str]) -> int:
    """Write rendered manifests to the output directory.

    Parameters
    ----------
    output_dir : Path
        Base directory for manifest output.
    manifests : dict[str, str]
        Map of relative paths to YAML content.

    Returns
    -------
    int
        Number of manifests written.
    """
    count = 0
    for rel_path, content in manifests.items():
        dest = output_dir / rel_path
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_text(content, encoding="utf-8")
        count += 1
    return count


def validate_cluster_name(name: str) -> str:
    """Validate and normalise a cluster name.

    Parameters
    ----------
    name : str
        Cluster name to validate.

    Returns
    -------
    str
        Normalised cluster name.

    Raises
    ------
    ValueError
        If the name is invalid.
    """
    import re

    name = name.strip().lower()
    if not name:
        msg = "cluster_name must not be blank"
        raise ValueError(msg)
    if not re.match(r"^[a-z0-9]([a-z0-9-]*[a-z0-9])?$", name):
        msg = "cluster_name must contain only lowercase letters, numbers, and hyphens"
        raise ValueError(msg)
    return name
