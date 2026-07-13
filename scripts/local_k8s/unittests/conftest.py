"""Shared fixtures for local_k8s unit tests."""

from __future__ import annotations

from collections.abc import Callable
import os
from pathlib import Path
from shutil import which
from types import SimpleNamespace

import pytest

from local_k8s.config import ContainerEngine, K8sProvider, PreviewConfig

type CommandRecord = tuple[str, list[str], str | None]
type RunHook = Callable[[str, list[str], str | None], None]


@pytest.fixture(autouse=True)
def clean_wildside_env(monkeypatch: pytest.MonkeyPatch) -> None:
    """Clear local preview environment variables before each test."""
    for name in tuple(os.environ):
        if not name.startswith("WILDSIDE_"):
            continue
        monkeypatch.delenv(name, raising=False)


@pytest.fixture(scope="session")
def uv_executable() -> str:
    """Return the uv executable used for CLI boundary tests."""
    uv = which("uv")
    assert uv is not None, "uv must be available to execute scripts/local_k8s.py"
    return uv


@pytest.fixture(scope="session")
def local_k8s_script() -> Path:
    """Return the repository-local local_k8s.py script path."""
    return Path(__file__).resolve().parents[2] / "local_k8s.py"


def _make_preview_config(
    *,
    container_engine: ContainerEngine = "docker",
    k8s_provider: K8sProvider = "k3d",
) -> PreviewConfig:
    """Return a representative PreviewConfig for unit testing."""
    return PreviewConfig(
        repository_root=Path("/repo"),
        container_engine=container_engine,
        k8s_provider=k8s_provider,
        cluster_name="wildside-preview",
        namespace="wildside",
        release_name="preview",
        image_name="wildside-backend:local",
        kind_node_image="kindest/node:v1.31.0",
        ingress_port=8088,
        chart_path=Path("/repo/deploy/charts/wildside"),
        local_values_path=Path("/repo/deploy/charts/wildside/values.local.yaml"),
        dockerfile_path=Path("/repo/deploy/docker/backend.Dockerfile"),
    )


@pytest.fixture
def preview_config() -> PreviewConfig:
    """Return a Docker plus k3d PreviewConfig for unit testing."""
    return _make_preview_config()


@pytest.fixture
def preview_config_kind() -> PreviewConfig:
    """Return a Podman plus kind PreviewConfig for unit testing."""
    return _make_preview_config(container_engine="podman", k8s_provider="kind")


def install_run_recorder(
    monkeypatch: pytest.MonkeyPatch,
    *,
    stdout: str = "",
    on_run: RunHook | None = None,
) -> list[CommandRecord]:
    """Replace deployment command execution with a command recorder.

    Parameters
    ----------
    monkeypatch : pytest.MonkeyPatch
        Pytest monkeypatch fixture used to replace ``local_k8s.deployment.run``.
    stdout : str, optional
        Standard output returned by every recorded command.
    on_run : RunHook | None, optional
        Callback invoked with the command, argument list, and optional input
        text before the command is recorded.

    Returns
    -------
    list[CommandRecord]
        Mutable command log populated with ``(command, args, input_text)``
        records for each deployment command invocation.
    """
    commands: list[CommandRecord] = []

    def record_run(command: str, args: list[str], **kwargs: object) -> SimpleNamespace:
        input_text = kwargs.get("input_text")
        if input_text is not None and not isinstance(input_text, str):
            error_message = "input_text must be text when provided"
            raise TypeError(error_message)
        if on_run is not None:
            on_run(command, args, input_text)
        commands.append((command, args, input_text))
        return SimpleNamespace(stdout=stdout)

    monkeypatch.setattr("local_k8s.deployment.run", record_run)
    return commands
