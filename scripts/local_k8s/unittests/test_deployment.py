"""Unit tests for local preview deployment orchestration.

These tests exercise the orchestration logic in ``local_k8s.deployment``
without invoking Kubernetes, Helm, k3d, or Docker. They document the preflight
contract for full build-and-deploy runs and the ``skip_build`` path used with
prebuilt images. The key invariant is that deployment tools depend on the
selected provider, while Docker or Podman is required only when the deployment
will build an image locally.
"""

from __future__ import annotations

import dataclasses as dc
import typing as typ

import pytest
from conftest import install_run_recorder
from local_k8s.deployment import (
    _deploy_preview_tools,
    build_image,
    deploy_preview,
    helm_upgrade,
)
from local_k8s.validation import LocalK8sError

if typ.TYPE_CHECKING:
    import collections.abc as cabc

    from local_k8s.config import K8sProvider, PreviewConfig


@pytest.mark.parametrize(
    ("skip_build", "expected_tools"),
    [
        (True, ("helm", "k3d", "kubectl")),
        (False, ("docker", "helm", "k3d", "kubectl")),
    ],
    ids=["skip-build", "build-image"],
)
def test_deploy_preview_docker_requirement_conditional_on_skip_build(
    monkeypatch: pytest.MonkeyPatch,
    preview_config: PreviewConfig,
    skip_build: bool,  # noqa: FBT001 - pytest parametrize documents both boolean modes.
    expected_tools: tuple[str, ...],
) -> None:
    """Verify that Docker preflight follows the selected build mode."""
    required_tools: list[tuple[str, ...]] = []
    calls: list[str] = []

    def record_step(name: str) -> cabc.Callable[[PreviewConfig], None]:
        """Return a side-effect replacement that records orchestration order."""

        def step(_: PreviewConfig) -> None:
            calls.append(name)

        return step

    monkeypatch.setattr(
        "local_k8s.deployment.require_tools",
        lambda tools: (
            calls.append("require_tools"),
            required_tools.append(tuple(tools)),
        ),
    )
    monkeypatch.setattr(
        "local_k8s.deployment.ensure_cluster", record_step("ensure_cluster")
    )
    monkeypatch.setattr(
        "local_k8s.deployment.ensure_namespace", record_step("ensure_namespace")
    )
    monkeypatch.setattr(
        "local_k8s.deployment.ensure_session_secret",
        record_step("ensure_session_secret"),
    )
    monkeypatch.setattr(
        "local_k8s.deployment.import_image", record_step("import_image")
    )
    monkeypatch.setattr(
        "local_k8s.deployment.helm_upgrade", record_step("helm_upgrade")
    )
    monkeypatch.setattr(
        "local_k8s.deployment.print_status", record_step("print_status")
    )
    monkeypatch.setattr("local_k8s.deployment.build_image", record_step("build_image"))

    deploy_preview(preview_config, skip_build=skip_build)

    assert required_tools == [expected_tools], (
        f"expected require_tools to be called once with {expected_tools}, "
        f"but got {required_tools}"
    )
    expected_calls = [
        "require_tools",
        "ensure_cluster",
        "ensure_namespace",
        "ensure_session_secret",
        *([] if skip_build else ["build_image"]),
        "import_image",
        "helm_upgrade",
        "print_status",
    ]
    assert calls == expected_calls, "deploy_preview must preserve lifecycle order"


def test_deploy_preview_tools_follow_configured_kubernetes_provider(
    preview_config: PreviewConfig,
) -> None:
    """Verify provider preflight follows the configured local cluster tool."""
    kind_config = dc.replace(preview_config, k8s_provider="kind")

    assert _deploy_preview_tools(kind_config, skip_build=True) == (
        "helm",
        "kind",
        "kubectl",
    ), "kind preflight must require the kind provider tool alongside helm and kubectl"


def test_deploy_preview_tools_reject_unexpected_kubernetes_provider(
    preview_config: PreviewConfig,
) -> None:
    """Verify provider preflight rejects impossible provider values."""
    invalid_config = dc.replace(
        preview_config, k8s_provider=typ.cast("K8sProvider", "minikube")
    )

    with pytest.raises(LocalK8sError, match="Unsupported Kubernetes provider"):
        _deploy_preview_tools(invalid_config, skip_build=True)


def test_build_image_uses_configured_container_engine(
    monkeypatch: pytest.MonkeyPatch,
    preview_config: PreviewConfig,
) -> None:
    """Verify local image builds use Docker or Podman from configuration."""
    # Podman is only supported with the kind provider, so pair the engines.
    podman_config = dc.replace(
        preview_config, container_engine="podman", k8s_provider="kind"
    )
    commands = install_run_recorder(monkeypatch)

    build_image(podman_config)

    assert commands == [
        (
            "podman",
            [
                "build",
                "-f",
                "/repo/deploy/docker/backend.Dockerfile",
                "-t",
                "wildside-backend:local",
                "/repo",
            ],
            None,
        )
    ], "image builds must use the configured container engine"


def test_helm_upgrade_uses_configured_kube_context(
    monkeypatch: pytest.MonkeyPatch,
    preview_config: PreviewConfig,
) -> None:
    """Verify Helm upgrades target the selected provider context."""
    config = dc.replace(preview_config, k8s_provider="kind")
    commands = install_run_recorder(monkeypatch)

    helm_upgrade(config)

    assert commands == [
        (
            "helm",
            [
                "--kube-context",
                "kind-wildside-preview",
                "upgrade",
                "--install",
                "preview",
                "/repo/deploy/charts/wildside",
                "--namespace",
                "wildside",
                "--values",
                "/repo/deploy/charts/wildside/values.local.yaml",
                "--set-string",
                "image.repository=wildside-backend",
                "--set-string",
                "image.tag=local",
                "--wait",
                "--timeout",
                "5m",
            ],
            None,
        )
    ], "Helm upgrades must use the provider-specific kube context"
