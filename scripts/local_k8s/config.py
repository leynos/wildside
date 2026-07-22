"""Configuration for the local Kubernetes Wildside preview."""

from __future__ import annotations

import dataclasses as dc
import os
import re
import typing as typ
from pathlib import Path

from .validation import LocalK8sError, validate_port

DEFAULT_CLUSTER_NAME = "wildside-preview"
DEFAULT_NAMESPACE = "wildside"
DEFAULT_RELEASE_NAME = "wildside"
DEFAULT_IMAGE_NAME = "wildside-backend:local"
DEFAULT_INGRESS_PORT = 8088
DEFAULT_CONTAINER_ENGINE = "docker"
DEFAULT_K8S_PROVIDER = "k3d"
DEFAULT_KIND_NODE_IMAGE = "kindest/node:v1.31.0"
DNS_1123_LABEL_PATTERN = re.compile(r"[a-z0-9](?:[-a-z0-9]{0,61}[a-z0-9])?")
# Helm release names must be DNS-1123 labels capped at 53 characters so the
# generated resource names leave room for chart-appended suffixes.
HELM_RELEASE_NAME_PATTERN = re.compile(r"[a-z0-9](?:[-a-z0-9]{0,51}[a-z0-9])?")
KIND_NODE_IMAGE_PATTERN = re.compile(r"[A-Za-z0-9][A-Za-z0-9._:/@_+-]{0,254}")

type ContainerEngine = typ.Literal["docker", "podman"]
type K8sProvider = typ.Literal["k3d", "kind"]


@dc.dataclass(frozen=True, slots=True)
class PreviewConfig:
    """Repository-local configuration for a Wildside preview deployment.

    Attributes
    ----------
    repository_root : Path
        Root directory of the Wildside checkout.
    container_engine : ContainerEngine
        Container runtime used to build and import the preview image.
    k8s_provider : K8sProvider
        Local Kubernetes provider used for the preview cluster.
    cluster_name : str
        Name of the local Kubernetes cluster.
    namespace : str
        Kubernetes namespace used by the preview release.
    release_name : str
        Helm release name for the preview deployment.
    image_name : str
        Fully tagged image reference built for the preview.
    kind_node_image : str
        Node image used when creating a kind-backed cluster.
    ingress_port : int
        Host port used for preview ingress or port-forwarding.
    chart_path : Path
        Path to the Wildside Helm chart.
    local_values_path : Path
        Path to the local preview Helm values file.
    dockerfile_path : Path
        Path to the backend Dockerfile.
    """

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
        _validate_dns_1123_label(self.cluster_name, name="WILDSIDE_K8S_CLUSTER")
        _validate_dns_1123_label(self.namespace, name="WILDSIDE_K8S_NAMESPACE")
        _validate_helm_release_name(self.release_name)
        _validate_kind_node_image(self.kind_node_image)
        _validate_engine_provider_combination(self.container_engine, self.k8s_provider)

    @property
    def kube_context(self) -> str:
        """Return the kube context name created by the selected provider.

        Returns
        -------
        str
            The provider-prefixed kube context name, formed as
            ``{provider}-{cluster_name}``.
        """
        return f"{self.k8s_provider}-{self.cluster_name}"

    @classmethod
    def from_env(cls) -> PreviewConfig:
        """Build configuration from defaults and `WILDSIDE_` overrides.

        Returns
        -------
        PreviewConfig
            Configuration resolved from repository defaults overlaid with any
            ``WILDSIDE_`` environment variable overrides.
        """
        repository_root = Path(__file__).resolve().parents[2]
        container_engine = _container_engine_from_env()
        k8s_provider = _k8s_provider_from_env()
        ingress_port = _ingress_port_from_env()
        chart_path = repository_root / "deploy" / "charts" / "wildside"
        return cls(
            repository_root=repository_root,
            container_engine=container_engine,
            k8s_provider=k8s_provider,
            cluster_name=_cluster_name_from_env(),
            namespace=os.environ.get("WILDSIDE_K8S_NAMESPACE", DEFAULT_NAMESPACE),
            release_name=os.environ.get("WILDSIDE_HELM_RELEASE", DEFAULT_RELEASE_NAME),
            image_name=os.environ.get("WILDSIDE_IMAGE", DEFAULT_IMAGE_NAME),
            kind_node_image=_kind_node_image_from_env(),
            ingress_port=ingress_port,
            chart_path=chart_path,
            local_values_path=chart_path / "values.local.yaml",
            dockerfile_path=repository_root
            / "deploy"
            / "docker"
            / "backend.Dockerfile",
        )


def _container_engine_from_env() -> ContainerEngine:
    """Return the validated container engine environment value."""
    raw_value = os.environ.get("WILDSIDE_CONTAINER_ENGINE", DEFAULT_CONTAINER_ENGINE)
    if raw_value in {"docker", "podman"}:
        # A membership test does not narrow str to the Literal alias for ty.
        return typ.cast("ContainerEngine", raw_value)
    message = (
        f"WILDSIDE_CONTAINER_ENGINE must be one of docker, podman; got {raw_value!r}"
    )
    raise LocalK8sError(message)


def _k8s_provider_from_env() -> K8sProvider:
    """Return the validated Kubernetes provider environment value."""
    raw_value = os.environ.get("WILDSIDE_K8S_PROVIDER", DEFAULT_K8S_PROVIDER)
    if raw_value in {"k3d", "kind"}:
        # A membership test does not narrow str to the Literal alias for ty.
        return typ.cast("K8sProvider", raw_value)
    message = f"WILDSIDE_K8S_PROVIDER must be one of k3d, kind; got {raw_value!r}"
    raise LocalK8sError(message)


def _ingress_port_from_env() -> int:
    """Return the ingress port and attribute errors to the source variable."""
    port_name = "WILDSIDE_K8S_PORT"
    raw_value = os.environ.get(port_name)
    if raw_value is None:
        port_name = "WILDSIDE_K3D_PORT"
        raw_value = os.environ.get(port_name)
    return validate_port(
        raw_value,
        default=DEFAULT_INGRESS_PORT,
        name=port_name,
    )


def _cluster_name_from_env() -> str:
    """Return the cluster name and attribute errors to the source variable."""
    cluster_name = "WILDSIDE_K8S_CLUSTER"
    raw_value = os.environ.get(cluster_name)
    if raw_value is None:
        cluster_name = "WILDSIDE_K3D_CLUSTER"
        raw_value = os.environ.get(cluster_name, DEFAULT_CLUSTER_NAME)
    _validate_dns_1123_label(raw_value, name=cluster_name)
    return raw_value


def _kind_node_image_from_env() -> str:
    """Return the validated kind node image override."""
    raw_value = os.environ.get("WILDSIDE_KIND_NODE_IMAGE", DEFAULT_KIND_NODE_IMAGE)
    _validate_kind_node_image(raw_value)
    return raw_value


def _validate_engine_provider_combination(
    container_engine: ContainerEngine, k8s_provider: K8sProvider
) -> None:
    """Reject unsupported container-engine and provider pairings.

    Rootless Podman previews only support the ``kind`` provider; k3d has no
    rootless-Podman story, so the ``podman`` plus ``k3d`` pairing is refused.
    The supported combinations are ``docker`` plus ``k3d`` or ``kind`` and
    ``podman`` plus ``kind``.
    """
    if container_engine == "podman" and k8s_provider == "k3d":
        message = (
            "Podman previews require the kind provider; set "
            "WILDSIDE_K8S_PROVIDER=kind (k3d is not supported with Podman)"
        )
        raise LocalK8sError(message)


def _validate_dns_1123_label(value: str, *, name: str) -> None:
    """Reject values unsafe for Kubernetes DNS-1123 label fields."""
    if DNS_1123_LABEL_PATTERN.fullmatch(value) is None:
        message = (
            f"{name} must contain only lowercase letters, "
            "digits, and hyphens; start and end with an alphanumeric "
            "character; and be at most 63 characters"
        )
        raise LocalK8sError(message)


def _validate_helm_release_name(value: str) -> None:
    """Reject Helm release names unsafe for shell hints or resource naming."""
    if HELM_RELEASE_NAME_PATTERN.fullmatch(value) is None:
        message = (
            "WILDSIDE_HELM_RELEASE must contain only lowercase letters, "
            "digits, and hyphens; start and end with an alphanumeric "
            "character; and be at most 53 characters"
        )
        raise LocalK8sError(message)


def _validate_kind_node_image(value: str) -> None:
    """Reject kind node image values unsafe for YAML rendering."""
    if KIND_NODE_IMAGE_PATTERN.fullmatch(value) is None:
        message = (
            "WILDSIDE_KIND_NODE_IMAGE must be a non-empty image reference "
            "without whitespace or control characters"
        )
        raise LocalK8sError(message)
