"""Shared fixtures for local_k8s unit tests."""

from __future__ import annotations

from pathlib import Path

import pytest

from local_k8s.config import PreviewConfig


@pytest.fixture
def preview_config() -> PreviewConfig:
    """Return a representative PreviewConfig for unit testing."""
    return PreviewConfig(
        repository_root=Path("/repo"),
        cluster_name="wildside-preview",
        namespace="wildside",
        release_name="preview",
        image_name="wildside-backend:local",
        ingress_port=8088,
        chart_path=Path("/repo/deploy/charts/wildside"),
        local_values_path=Path("/repo/deploy/charts/wildside/values.local.yaml"),
        dockerfile_path=Path("/repo/deploy/docker/backend.Dockerfile"),
    )
