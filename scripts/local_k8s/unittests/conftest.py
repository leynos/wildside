"""Shared fixtures for local_k8s unit tests."""

from __future__ import annotations

from pathlib import Path

import pytest

from local_k8s.config import ContainerEngine, K8sProvider, PreviewConfig


@pytest.fixture(autouse=True)
def clean_wildside_env(monkeypatch: pytest.MonkeyPatch) -> None:
    """Clear local preview environment variables before each test."""
    for name in (
        "WILDSIDE_CONTAINER_ENGINE",
        "WILDSIDE_K8S_PROVIDER",
        "WILDSIDE_K8S_CLUSTER",
        "WILDSIDE_K8S_PORT",
        "WILDSIDE_K3D_CLUSTER",
        "WILDSIDE_K3D_PORT",
        "WILDSIDE_KIND_NODE_IMAGE",
    ):
        monkeypatch.delenv(name, raising=False)


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
