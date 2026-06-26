"""Configuration for the local Kubernetes Wildside preview."""

from __future__ import annotations

import os
import re
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
DEFAULT_KIND_NODE_IMAGE = "kindest/node:v1.31.0"
CLUSTER_NAME_PATTERN = re.compile(r"[a-z0-9](?:[-a-z0-9]{0,61}[a-z0-9])?")
KIND_NODE_IMAGE_PATTERN = re.compile(r"[A-Za-z0-9][A-Za-z0-9._:/@_+-]{0,254}")

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
    kind_node_image: str
    ingress_port: int
    chart_path: Path
    local_values_path: Path
    dockerfile_path: Path

    def __post_init__(self) -> None:
        """Validate fields that are later forwarded to paths or YAML."""
        _validate_cluster_name(self.cluster_name)
        _validate_kind_node_image(self.kind_node_image)

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
            cluster_name=_cluster_name_from_env(
                os.environ.get("WILDSIDE_K8S_CLUSTER")
                or os.environ.get("WILDSIDE_K3D_CLUSTER", DEFAULT_CLUSTER_NAME)
            ),
            namespace=os.environ.get("WILDSIDE_K8S_NAMESPACE", DEFAULT_NAMESPACE),
            release_name=os.environ.get("WILDSIDE_HELM_RELEASE", DEFAULT_RELEASE_NAME),
            image_name=os.environ.get("WILDSIDE_IMAGE", DEFAULT_IMAGE_NAME),
            kind_node_image=_kind_node_image_from_env(),
            ingress_port=ingress_port,
            chart_path=chart_path,
            local_values_path=chart_path / "values.local.yaml",
            dockerfile_path=repository_root / "deploy" / "docker" / "backend.Dockerfile",
        )


def _container_engine_from_env() -> ContainerEngine:
    """Return the validated container engine environment value."""
    raw_value = os.environ.get("WILDSIDE_CONTAINER_ENGINE", DEFAULT_CONTAINER_ENGINE)
    if raw_value in ("docker", "podman"):
        return raw_value
    raise LocalK8sError(
        "WILDSIDE_CONTAINER_ENGINE must be one of docker, podman; "
        f"got {raw_value!r}"
    )


def _k8s_provider_from_env() -> K8sProvider:
    """Return the validated Kubernetes provider environment value."""
    raw_value = os.environ.get("WILDSIDE_K8S_PROVIDER", DEFAULT_K8S_PROVIDER)
    if raw_value in ("k3d", "kind"):
        return raw_value
    raise LocalK8sError(
        "WILDSIDE_K8S_PROVIDER must be one of k3d, kind; "
        f"got {raw_value!r}"
    )


def _cluster_name_from_env(raw_value: str) -> str:
    """Return the validated local Kubernetes cluster name."""
    _validate_cluster_name(raw_value)
    return raw_value


def _kind_node_image_from_env() -> str:
    """Return the validated kind node image override."""
    raw_value = os.environ.get("WILDSIDE_KIND_NODE_IMAGE", DEFAULT_KIND_NODE_IMAGE)
    _validate_kind_node_image(raw_value)
    return raw_value


def _validate_cluster_name(value: str) -> None:
    """Reject cluster names unsafe for Kubernetes names and local paths."""
    if CLUSTER_NAME_PATTERN.fullmatch(value) is None:
        raise LocalK8sError(
            "WILDSIDE_K8S_CLUSTER must contain only lowercase letters, "
            "digits, and hyphens; start and end with an alphanumeric "
            "character; and be at most 63 characters"
        )


def _validate_kind_node_image(value: str) -> None:
    """Reject kind node image values unsafe for YAML rendering."""
    if KIND_NODE_IMAGE_PATTERN.fullmatch(value) is None:
        raise LocalK8sError(
            "WILDSIDE_KIND_NODE_IMAGE must be a non-empty image reference "
            "without whitespace or control characters"
        )
