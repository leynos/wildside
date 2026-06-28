"""Validation helpers for local Kubernetes preview commands."""

from __future__ import annotations

from collections.abc import Iterable
from shutil import which

from cuprum import ProgramCatalogue as Catalogue
from cuprum import ProjectSettings, UnknownProgramError, sh


class LocalK8sError(RuntimeError):
    """Raised when a local preview preflight or command fails."""


def validate_port(raw_value: str | None, *, default: int, name: str) -> int:
    """Return a TCP port from an optional environment variable value."""

    if raw_value is None or raw_value == "":
        return default
    try:
        port = int(raw_value)
    except ValueError as exc:
        raise LocalK8sError(f"{name} must be an integer TCP port") from exc
    if not 1 <= port <= 65535:
        raise LocalK8sError(f"{name} must be between 1 and 65535")
    return port


def require_tools(tools: Iterable[str]) -> None:
    """Fail with a concise preflight error when required executables are absent."""

    missing = [tool for tool in tools if _is_missing(tool)]
    if missing:
        joined = ", ".join(missing)
        raise LocalK8sError(f"missing required executable(s): {joined}")


def _is_missing(tool: str) -> bool:
    """Return True when an executable cannot be resolved on PATH."""
    try:
        sh.make(tool, catalogue=_catalogue_for(tool))
    except UnknownProgramError:
        return True
    return which(tool) is None


def _catalogue_for(tool: str) -> Catalogue:
    """Return a one-tool catalogue for preflight command construction."""
    return Catalogue(
        projects=(
            ProjectSettings(
                name="local-k8s-preflight",
                programs=(tool,),
                documentation_locations=(),
                noise_rules=(),
            ),
        )
    )
