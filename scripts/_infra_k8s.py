"""Shared utilities for the wildside-infra-k8s action.

This module provides helpers for OpenTofu invocation, backend configuration,
manifest handling, and GitHub Action context management.
"""

from __future__ import annotations

import json
import os
import re
import subprocess
from collections import abc as cabc
from collections.abc import Mapping
from dataclasses import dataclass
from pathlib import Path


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
class TofuResult:
    """Result of an OpenTofu command execution."""

    success: bool
    stdout: str
    stderr: str
    return_code: int


def mask_secret(value: str, stream: cabc.Callable[[str], object] = print) -> None:
    """Emit GitHub Action secret masking command.

    Parameters
    ----------
    value : str
        Secret value to mask.
    stream : Callable[[str], object], optional
        Output stream for the masking command (defaults to ``print``).

    Returns
    -------
    None
        Writes the masking command when ``value`` is non-empty.

    Examples
    --------
    >>> mask_secret("token")
    """
    if value:
        stream(f"::add-mask::{value}")


def parse_bool(value: str | None, *, default: bool = True) -> bool:
    """Parse a boolean string value.

    Parameters
    ----------
    value : str | None
        Raw string value to parse.
    default : bool, optional
        Default value when ``value`` is ``None``.

    Returns
    -------
    bool
        Parsed boolean value.

    Examples
    --------
    >>> parse_bool("yes")
    True
    >>> parse_bool(None, default=False)
    False
    """
    if value is None:
        return default
    return value.strip().lower() in ("true", "1", "yes")


def parse_node_pools(value: str | None) -> list[dict[str, object]] | None:
    """Parse node pools JSON string.

    Parameters
    ----------
    value : str | None
        JSON-encoded node pool payload.

    Returns
    -------
    list[dict[str, object]] | None
        Parsed node pool structures, or ``None`` if input is blank.

    Examples
    --------
    >>> parse_node_pools('[{"name": "default", "node_count": 2}]')[0]["name"]
    'default'
    """
    if value is None or not value.strip():
        return None
    try:
        pools = json.loads(value)
    except json.JSONDecodeError as exc:
        msg = f"Invalid JSON in node_pools: {exc}"
        raise ValueError(msg) from exc
    else:
        if not isinstance(pools, list):
            msg = "node_pools must be a JSON array"
            raise TypeError(msg)
        return pools


def _choose_multiline_delimiter(value: str, base: str = "EOF") -> str:
    """Choose a heredoc delimiter that is not in the value."""
    delimiter = base
    counter = 0
    while delimiter in value:
        counter += 1
        delimiter = f"{base}_{counter}"
    return delimiter


def _write_github_multiline(
    handle: cabc.Callable[[str], int],
    key: str,
    value: str,
) -> None:
    """Write a multiline GitHub Actions value using heredoc syntax."""
    delimiter = _choose_multiline_delimiter(value)
    handle(f"{key}<<{delimiter}\n")
    handle(f"{value}\n")
    handle(f"{delimiter}\n")


def _append_github_kv(target_file: Path, items: Mapping[str, str]) -> None:
    """Append key-value pairs to a GitHub Actions metadata file."""
    target_file.parent.mkdir(parents=True, exist_ok=True)
    with target_file.open("a", encoding="utf-8") as handle:
        for key, value in items.items():
            if "\n" in value or "\r" in value:
                _write_github_multiline(handle.write, key, value)
            else:
                handle.write(f"{key}={value}\n")


def append_github_env(env_file: Path, variables: dict[str, str]) -> None:
    """Append environment variables to GITHUB_ENV file.

    Parameters
    ----------
    env_file : Path
        Path to the ``GITHUB_ENV`` file.
    variables : dict[str, str]
        Environment variables to append.

    Returns
    -------
    None
        Writes entries to the ``GITHUB_ENV`` file.

    Examples
    --------
    >>> append_github_env(Path("/tmp/env"), {"CLUSTER_NAME": "preview-1"})
    """
    _append_github_kv(env_file, variables)


def append_github_output(output_file: Path, outputs: dict[str, str]) -> None:
    """Append outputs to GITHUB_OUTPUT file.

    Parameters
    ----------
    output_file : Path
        Path to the ``GITHUB_OUTPUT`` file.
    outputs : dict[str, str]
        Outputs to append.

    Returns
    -------
    None
        Writes entries to the ``GITHUB_OUTPUT`` file.

    Examples
    --------
    >>> append_github_output(Path("/tmp/out"), {"cluster_id": "abc"})
    """
    _append_github_kv(output_file, outputs)


def _validate_command_args(args: list[str]) -> None:
    """Validate OpenTofu CLI arguments for safe execution."""
    for arg in args:
        if not isinstance(arg, str):
            msg = f"OpenTofu argument must be a string, got {type(arg).__name__}"
            raise TypeError(msg)
        if any(char in arg for char in ("\x00", "\n", "\r")):
            msg = "OpenTofu argument contains an invalid control character"
            raise ValueError(msg)


def run_tofu(
    args: list[str],
    cwd: Path,
    env: cabc.Mapping[str, str] | None = None,
    *,
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

    # Security: list-based invocation without shell=True is safe from injection.
    # Validate arguments to avoid unexpected control characters.
    _validate_command_args(cmd)
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
    """Initialize OpenTofu with backend configuration.

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
        f"-backend-config=region={backend_config.region}",
        f"-backend-config=endpoint={backend_config.endpoint}",
        f"-backend-config=key={backend_config.state_key}",
        "-input=false",
    ]
    backend_env = {
        "AWS_ACCESS_KEY_ID": backend_config.access_key,
        "AWS_SECRET_ACCESS_KEY": backend_config.secret_key,
    }
    return run_tofu(args, cwd, env=backend_env)


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
    *,
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


def tofu_output(cwd: Path, name: str | None = None) -> object:
    """Retrieve OpenTofu outputs as JSON.

    Parameters
    ----------
    cwd : Path
        OpenTofu configuration directory.
    name : str | None, optional
        Specific output name to retrieve.

    Returns
    -------
    object
        Parsed JSON output. If name is specified, returns the raw value.
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
    output_root = output_dir.resolve()
    for rel_path, content in manifests.items():
        rel = Path(rel_path)
        if rel.is_absolute() or ".." in rel.parts:
            msg = f"Refusing to write manifest outside {output_dir}"
            raise ValueError(msg)
        dest = output_dir / rel
        if not dest.resolve().is_relative_to(output_root):
            msg = f"Refusing to write manifest outside {output_dir}"
            raise ValueError(msg)
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_text(content, encoding="utf-8")
        count += 1
    return count


def validate_cluster_name(name: str) -> str:
    """Validate and normalize a cluster name.

    Parameters
    ----------
    name : str
        Cluster name to validate.

    Returns
    -------
    str
        Normalized cluster name.

    Raises
    ------
    ValueError
        If the name is invalid.
    """
    name = name.strip().lower()
    if not name:
        msg = "cluster_name must not be blank"
        raise ValueError(msg)
    if not re.match(r"^[a-z0-9]([a-z0-9-]*[a-z0-9])?$", name):
        msg = "cluster_name must contain only lowercase letters, numbers, and hyphens"
        raise ValueError(msg)
    return name
