"""Unit tests for local preview configuration parsing.

Covers ``PreviewConfig.from_env()`` behaviour including default Docker plus
``k3d`` fallback, provider-neutral ``WILDSIDE_K8S_*`` overrides, legacy
``WILDSIDE_K3D_*`` alias support, and validation errors for unsupported
provider values.
"""

from __future__ import annotations

import os

import pytest
from local_k8s.config import PreviewConfig
from local_k8s.validation import LocalK8sError

UNSAFE_KUBERNETES_NAMES = [
    "../wildside",
    "Wildside",
    "wildside_",
    "-wildside",
    "wildside-",
]


def test_wildside_environment_is_isolated() -> None:
    """Verify local preview tests start without inherited WILDSIDE variables."""
    leaked = [name for name in os.environ if name.startswith("WILDSIDE_")]
    assert not leaked, f"expected no inherited WILDSIDE_ variables, got {leaked}"


def test_preview_config_uses_provider_neutral_defaults(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Verify the default local preview mode remains Docker plus k3d."""
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

    config = PreviewConfig.from_env()

    assert config.container_engine == "docker", (
        "default container engine must be Docker"
    )
    assert config.k8s_provider == "k3d", "default Kubernetes provider must be k3d"
    assert config.cluster_name == "wildside-preview", (
        "default cluster name must be wildside-preview"
    )
    assert config.ingress_port == 8088, "default ingress port must be 8088"
    assert config.kind_node_image == "kindest/node:v1.31.0", (
        "default kind node image must satisfy the Helm chart kubeVersion range"
    )


def test_preview_config_accepts_podman_kind_env(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Verify provider-neutral environment variables configure kind previews."""
    monkeypatch.setenv("WILDSIDE_CONTAINER_ENGINE", "podman")
    monkeypatch.setenv("WILDSIDE_K8S_PROVIDER", "kind")
    monkeypatch.setenv("WILDSIDE_K8S_CLUSTER", "wildside-kind")
    monkeypatch.setenv("WILDSIDE_K8S_PORT", "18088")
    monkeypatch.setenv("WILDSIDE_K3D_CLUSTER", "legacy-cluster")
    monkeypatch.setenv("WILDSIDE_K3D_PORT", "28088")

    config = PreviewConfig.from_env()

    assert config.container_engine == "podman", (
        "WILDSIDE_CONTAINER_ENGINE must override the default"
    )
    assert config.k8s_provider == "kind", (
        "WILDSIDE_K8S_PROVIDER must override the default"
    )
    assert config.cluster_name == "wildside-kind", (
        "WILDSIDE_K8S_CLUSTER must override legacy aliases"
    )
    assert config.ingress_port == 18088, (
        "WILDSIDE_K8S_PORT must override legacy aliases"
    )


def test_preview_config_accepts_kind_node_image_override(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Verify kind node image can be overridden for local Kubernetes upgrades."""
    monkeypatch.setenv("WILDSIDE_KIND_NODE_IMAGE", "kindest/node:v1.31.9")

    config = PreviewConfig.from_env()

    assert config.kind_node_image == "kindest/node:v1.31.9", (
        "WILDSIDE_KIND_NODE_IMAGE must override the default kind node image"
    )


def test_preview_config_uses_legacy_k3d_aliases(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Verify legacy k3d-specific variables still work when new names are unset."""
    monkeypatch.delenv("WILDSIDE_K8S_CLUSTER", raising=False)
    monkeypatch.delenv("WILDSIDE_K8S_PORT", raising=False)
    monkeypatch.setenv("WILDSIDE_K3D_CLUSTER", "legacy-cluster")
    monkeypatch.setenv("WILDSIDE_K3D_PORT", "28088")

    config = PreviewConfig.from_env()

    assert config.cluster_name == "legacy-cluster", (
        "WILDSIDE_K3D_CLUSTER must remain a legacy alias"
    )
    assert config.ingress_port == 28088, "WILDSIDE_K3D_PORT must remain a legacy alias"


def test_preview_config_attributes_legacy_port_validation_errors(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Verify legacy port validation errors name the legacy variable."""
    monkeypatch.delenv("WILDSIDE_K8S_PORT", raising=False)
    monkeypatch.setenv("WILDSIDE_K3D_PORT", "not-a-port")

    with pytest.raises(LocalK8sError, match="WILDSIDE_K3D_PORT"):
        PreviewConfig.from_env()


def test_preview_config_attributes_legacy_cluster_validation_errors(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Verify legacy cluster-name validation errors name the legacy variable."""
    monkeypatch.delenv("WILDSIDE_K8S_CLUSTER", raising=False)
    monkeypatch.setenv("WILDSIDE_K3D_CLUSTER", "Invalid_Name")

    with pytest.raises(LocalK8sError, match="WILDSIDE_K3D_CLUSTER"):
        PreviewConfig.from_env()


@pytest.mark.parametrize(
    ("env_name", "env_value"),
    [
        ("WILDSIDE_CONTAINER_ENGINE", "containerd"),
        ("WILDSIDE_K8S_PROVIDER", "minikube"),
    ],
)
def test_preview_config_rejects_unknown_provider_values(
    monkeypatch: pytest.MonkeyPatch,
    env_name: str,
    env_value: str,
) -> None:
    """Verify provider fields fail fast for unsupported values."""
    monkeypatch.setenv(env_name, env_value)

    with pytest.raises(LocalK8sError, match=env_value):
        PreviewConfig.from_env()


def test_preview_config_rejects_podman_with_k3d(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Verify the unsupported Podman plus k3d pairing fails fast."""
    monkeypatch.setenv("WILDSIDE_CONTAINER_ENGINE", "podman")
    monkeypatch.setenv("WILDSIDE_K8S_PROVIDER", "k3d")

    with pytest.raises(LocalK8sError, match="Podman previews require"):
        PreviewConfig.from_env()


@pytest.mark.parametrize(
    "cluster_name",
    UNSAFE_KUBERNETES_NAMES,
)
def test_preview_config_rejects_unsafe_cluster_names(
    monkeypatch: pytest.MonkeyPatch,
    cluster_name: str,
) -> None:
    """Verify cluster names cannot escape temp paths or Kubernetes naming."""
    monkeypatch.setenv("WILDSIDE_K8S_CLUSTER", cluster_name)

    with pytest.raises(LocalK8sError, match="WILDSIDE_K8S_CLUSTER"):
        PreviewConfig.from_env()


@pytest.mark.parametrize(
    "namespace",
    UNSAFE_KUBERNETES_NAMES,
)
def test_preview_config_rejects_unsafe_namespaces(
    monkeypatch: pytest.MonkeyPatch,
    namespace: str,
) -> None:
    """Verify namespace values cannot inject invalid Kubernetes YAML names."""
    monkeypatch.setenv("WILDSIDE_K8S_NAMESPACE", namespace)

    with pytest.raises(LocalK8sError, match="WILDSIDE_K8S_NAMESPACE"):
        PreviewConfig.from_env()


@pytest.mark.parametrize(
    "kind_node_image",
    ["kindest/node:v1.31.0\nextraPortMappings: []", " kindest/node:v1.31.0", ""],
)
def test_preview_config_rejects_unsafe_kind_node_images(
    monkeypatch: pytest.MonkeyPatch,
    kind_node_image: str,
) -> None:
    """Verify kind node image overrides cannot inject extra YAML content."""
    monkeypatch.setenv("WILDSIDE_KIND_NODE_IMAGE", kind_node_image)

    with pytest.raises(LocalK8sError, match="WILDSIDE_KIND_NODE_IMAGE"):
        PreviewConfig.from_env()
