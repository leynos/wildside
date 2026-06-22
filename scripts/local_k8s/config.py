"""Configuration for the local Kubernetes Wildside preview."""

from __future__ import annotations

import os
from dataclasses import dataclass
from pathlib import Path
from typing import Literal

from .validation import LocalK8sError, validate_port

DEFAULT_CLUSTER_NAME = "wildside-preview"
DEFAULT_NAMESPACE = "wildside"
DEFAULT_RELEASE_NAME = "wildside"
DEFAULT_IMAGE_NAME = "wildside-backend:local"
DEFAULT_INGRESS_PORT = 8088
DEFAULT_CONTAINER_ENGINE = "docker"
DEFAULT_K8S_PROVIDER = "k3d"

ContainerEngine = Literal["docker", "podman"]
K8sProvider = Literal["k3d", "kind"]


@dataclass(frozen=True, slots=True)
class PreviewConfig:
    """Repository-local configuration for a Wildside preview deployment."""

    repository_root: Path
    container_engine: ContainerEngine
    k8s_provider: K8sProvider
    cluster_name: str
    namespace: str
    release_name: str
    image_name: str
    ingress_port: int
    chart_path: Path
    local_values_path: Path
    dockerfile_path: Path

    @property
    def kube_context(self) -> str:
        """Return the kube context name created by the selected provider."""

        return f"{self.k8s_provider}-{self.cluster_name}"

    @classmethod
    def from_env(cls) -> "PreviewConfig":
        """Build configuration from defaults and `WILDSIDE_` overrides."""

        repository_root = Path(__file__).resolve().parents[2]
        container_engine = _container_engine_from_env()
        k8s_provider = _k8s_provider_from_env()
        ingress_port = validate_port(
            os.environ.get("WILDSIDE_K8S_PORT") or os.environ.get("WILDSIDE_K3D_PORT"),
            default=DEFAULT_INGRESS_PORT,
            name="WILDSIDE_K8S_PORT",
        )
        chart_path = repository_root / "deploy" / "charts" / "wildside"
        return cls(
            repository_root=repository_root,
            container_engine=container_engine,
            k8s_provider=k8s_provider,
            cluster_name=(
                os.environ.get("WILDSIDE_K8S_CLUSTER")
                or os.environ.get("WILDSIDE_K3D_CLUSTER", DEFAULT_CLUSTER_NAME)
            ),
            namespace=os.environ.get("WILDSIDE_K8S_NAMESPACE", DEFAULT_NAMESPACE),
            release_name=os.environ.get("WILDSIDE_HELM_RELEASE", DEFAULT_RELEASE_NAME),
            image_name=os.environ.get("WILDSIDE_IMAGE", DEFAULT_IMAGE_NAME),
            ingress_port=ingress_port,
            chart_path=chart_path,
            local_values_path=chart_path / "values.local.yaml",
            dockerfile_path=repository_root / "deploy" / "docker" / "backend.Dockerfile",
        )


def _container_engine_from_env() -> ContainerEngine:
    raw_value = os.environ.get("WILDSIDE_CONTAINER_ENGINE", DEFAULT_CONTAINER_ENGINE)
    if raw_value in ("docker", "podman"):
        return raw_value
    raise LocalK8sError(
        "WILDSIDE_CONTAINER_ENGINE must be one of docker, podman; "
        f"got {raw_value!r}"
    )


def _k8s_provider_from_env() -> K8sProvider:
    raw_value = os.environ.get("WILDSIDE_K8S_PROVIDER", DEFAULT_K8S_PROVIDER)
    if raw_value in ("k3d", "kind"):
        return raw_value
    raise LocalK8sError(
        "WILDSIDE_K8S_PROVIDER must be one of k3d, kind; "
        f"got {raw_value!r}"
    )
