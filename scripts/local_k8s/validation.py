"""Validation helpers for local Kubernetes preview commands."""

from __future__ import annotations

from collections.abc import Iterable
from shutil import which


class LocalK8sError(RuntimeError):
    """Raised when a local preview preflight or command fails.

    Optionally carries the raw command ``stderr`` and ``returncode`` so callers
    can classify failures structurally (for example, distinguishing a genuine
    Kubernetes ``AlreadyExists`` server conflict from an incidental mention of
    "already exists" in an unrelated error message) rather than inspecting the
    formatted string representation.
    """

    def __init__(
        self,
        message: str,
        *,
        stderr: str | None = None,
        returncode: int | None = None,
    ) -> None:
        super().__init__(message)
        self.stderr = stderr
        self.returncode = returncode


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
    return which(tool) is None
