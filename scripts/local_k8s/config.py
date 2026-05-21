"""Configuration for the local k3d Wildside preview."""

from __future__ import annotations

import os
from dataclasses import dataclass
from pathlib import Path

from .validation import validate_port

DEFAULT_CLUSTER_NAME = "wildside-preview"
DEFAULT_NAMESPACE = "wildside"
DEFAULT_RELEASE_NAME = "wildside"
DEFAULT_IMAGE_NAME = "wildside-backend:local"
DEFAULT_INGRESS_PORT = 8088


@dataclass(frozen=True, slots=True)
class PreviewConfig:
    """Repository-local configuration for a Wildside preview deployment."""

    repository_root: Path
    cluster_name: str
    namespace: str
    release_name: str
    image_name: str
    ingress_port: int
    chart_path: Path
    local_values_path: Path
    dockerfile_path: Path

    @classmethod
    def from_env(cls) -> "PreviewConfig":
        """Build configuration from defaults and `WILDSIDE_` overrides."""

        repository_root = Path(__file__).resolve().parents[2]
        ingress_port = validate_port(
            os.environ.get("WILDSIDE_K3D_PORT"),
            default=DEFAULT_INGRESS_PORT,
            name="WILDSIDE_K3D_PORT",
        )
        chart_path = repository_root / "deploy" / "charts" / "wildside"
        return cls(
            repository_root=repository_root,
            cluster_name=os.environ.get("WILDSIDE_K3D_CLUSTER", DEFAULT_CLUSTER_NAME),
            namespace=os.environ.get("WILDSIDE_K8S_NAMESPACE", DEFAULT_NAMESPACE),
            release_name=os.environ.get("WILDSIDE_HELM_RELEASE", DEFAULT_RELEASE_NAME),
            image_name=os.environ.get("WILDSIDE_IMAGE", DEFAULT_IMAGE_NAME),
            ingress_port=ingress_port,
            chart_path=chart_path,
            local_values_path=chart_path / "values.local.yaml",
            dockerfile_path=repository_root / "deploy" / "docker" / "backend.Dockerfile",
        )
