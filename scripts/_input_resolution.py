"""Shared helpers for resolving CLI and environment inputs."""

from __future__ import annotations

import os
from dataclasses import dataclass
from pathlib import Path
from collections import abc as cabc


@dataclass(frozen=True, slots=True)
class InputResolution:
    """Configuration for resolving an input from multiple sources."""

    env_key: str
    default: str | Path | None = None
    required: bool = False
    as_path: bool = False


def resolve_input(
    param_value: str | Path | None,
    resolution: InputResolution,
    env: cabc.Mapping[str, str] | None = None,
) -> str | Path | None:
    """Resolve input from parameter, environment variable, or default."""

    if param_value is not None:
        return param_value

    env_value = (env or os.environ).get(resolution.env_key)
    if env_value is not None:
        return Path(env_value) if resolution.as_path else env_value

    if resolution.required:
        msg = f"{resolution.env_key} is required"
        raise SystemExit(msg)

    return resolution.default
