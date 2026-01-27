"""Shared helpers for resolving CLI and environment inputs."""

from __future__ import annotations

import os
from collections import abc as cabc
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True, slots=True)
class InputResolution:
    """Configuration for resolving an input from multiple sources."""

    env_key: str
    default: str | Path | None = None
    required: bool = False
    as_path: bool = False


def _is_cyclopts_parameter(value: object) -> bool:
    """Return True when the value is a cyclopts Parameter sentinel."""
    cls = value.__class__
    return cls.__name__ == "Parameter" and cls.__module__.startswith("cyclopts")


def resolve_input(
    param_value: str | Path | None,
    resolution: InputResolution,
    env: cabc.Mapping[str, str] | None = None,
) -> str | Path | None:
    """Resolve input from parameter, environment variable, or default."""

    if param_value is not None and not _is_cyclopts_parameter(param_value):
        return param_value

    env_value = (env or os.environ).get(resolution.env_key)
    if env_value is not None:
        return Path(env_value) if resolution.as_path else env_value

    if resolution.required:
        msg = f"{resolution.env_key} is required"
        raise SystemExit(msg)

    return resolution.default
