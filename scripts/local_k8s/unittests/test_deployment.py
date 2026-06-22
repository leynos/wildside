"""Unit tests for local preview deployment orchestration.

These tests exercise the orchestration logic in ``local_k8s.deployment``
without invoking Kubernetes, Helm, k3d, or Docker. They document the preflight
contract for full build-and-deploy runs and the ``skip_build`` path used with
prebuilt images. The key invariant is that Docker is required only when the
deployment will build an image locally; Helm, k3d, and kubectl remain required
for both deployment modes.
"""

from __future__ import annotations

import base64
from dataclasses import replace
from pathlib import Path
from types import SimpleNamespace

import pytest

from local_k8s.config import PreviewConfig
from local_k8s.deployment import (
    _deploy_preview_tools,
    build_image,
    deploy_preview,
    ensure_session_secret,
    print_logs,
    print_status,
)


@pytest.fixture
def preview_config() -> PreviewConfig:
    """Representative local preview configuration for deployment tests.

    Returns
    -------
    PreviewConfig
        Configuration for a local preview release named ``preview`` in the
        ``wildside`` namespace. The image tag, chart path, values path, and
        Dockerfile path match the deployment fields that ``deploy_preview``
        passes through its build, import, and Helm orchestration steps.
    """

    return PreviewConfig(
        repository_root=Path("/repo"),
        container_engine="docker",
        k8s_provider="k3d",
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

    def no_op(_: PreviewConfig) -> None:
        """Replace deployment side effects during preflight assertions."""

    monkeypatch.setattr(
        "local_k8s.deployment.require_tools",
        lambda tools: required_tools.append(tuple(tools)),
    )
    monkeypatch.setattr("local_k8s.deployment.ensure_cluster", no_op)
    monkeypatch.setattr("local_k8s.deployment.ensure_namespace", no_op)
    monkeypatch.setattr("local_k8s.deployment.ensure_session_secret", no_op)
    monkeypatch.setattr("local_k8s.deployment.import_image", no_op)
    monkeypatch.setattr("local_k8s.deployment.helm_upgrade", no_op)
    monkeypatch.setattr("local_k8s.deployment.print_status", no_op)
    monkeypatch.setattr("local_k8s.deployment.build_image", no_op)

    deploy_preview(preview_config, skip_build=skip_build)

    assert required_tools == [expected_tools], (
        f"expected require_tools to be called once with {expected_tools}, "
        f"but got {required_tools}"
    )


def test_deploy_preview_tools_follow_configured_kubernetes_provider(
    preview_config: PreviewConfig,
) -> None:
    """Verify provider preflight follows the configured local cluster tool."""
    kind_config = replace(preview_config, k8s_provider="kind")

    assert _deploy_preview_tools(kind_config, skip_build=True) == (
        "helm",
        "kind",
        "kubectl",
    )


def test_build_image_uses_configured_container_engine(
    monkeypatch: pytest.MonkeyPatch,
    preview_config: PreviewConfig,
) -> None:
    """Verify local image builds use Docker or Podman from configuration."""
    podman_config = replace(preview_config, container_engine="podman")
    commands: list[tuple[str, list[str]]] = []

    def record_run(command: str, args: list[str], **_: object) -> None:
        commands.append((command, args))

    monkeypatch.setattr("local_k8s.deployment.run", record_run)

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
        )
    ], "image builds must use the configured container engine"


def test_ensure_session_secret_applies_runtime_key_manifest(
    monkeypatch: pytest.MonkeyPatch,
    preview_config: PreviewConfig,
) -> None:
    """Verify local preview creates a mounted session signing key Secret."""
    commands: list[tuple[str, list[str], str | None]] = []

    def record_run(command: str, args: list[str], **kwargs: object) -> None:
        commands.append((command, args, kwargs.get("input_text")))

    monkeypatch.setattr("local_k8s.deployment.run", record_run)
    monkeypatch.setattr(
        "local_k8s.deployment.secrets.token_bytes",
        lambda length: b"a" * length,
    )

    ensure_session_secret(preview_config)

    assert commands[0][0:2] == (
        "kubectl",
        [
            "--context",
            "k3d-wildside-preview",
            "apply",
            "-f",
            "-",
        ],
    ), "local preview must apply the session Secret before Helm"
    manifest = commands[0][2]
    assert manifest is not None
    assert "name: wildside-session-key" in manifest
    assert "namespace: wildside" in manifest
    encoded_key = manifest.rsplit("session_key: ", maxsplit=1)[1].strip()
    assert base64.b64decode(encoded_key) == b"a" * 96


def test_print_status_uses_provider_context_and_prints_kind_port_forward(
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    preview_config: PreviewConfig,
) -> None:
    """Verify kind status uses provider tools and prints the operator command."""
    config = replace(preview_config, k8s_provider="kind")
    required_tools: list[tuple[str, ...]] = []
    commands: list[tuple[str, list[str]]] = []

    def record_run(command: str, args: list[str], **_: object) -> SimpleNamespace:
        commands.append((command, args))
        return SimpleNamespace(stdout="helm status\n")

    monkeypatch.setattr(
        "local_k8s.deployment.require_tools",
        lambda tools: required_tools.append(tuple(tools)),
    )
    monkeypatch.setattr("local_k8s.deployment.print_cluster_status", lambda _: None)
    monkeypatch.setattr("local_k8s.deployment.print_kubernetes_status", lambda _: None)
    monkeypatch.setattr("local_k8s.deployment.run", record_run)

    print_status(config)

    output = capsys.readouterr().out
    assert required_tools == [("helm", "kind", "kubectl")]
    assert commands == [
        (
            "helm",
            [
                "--kube-context",
                "kind-wildside-preview",
                "-n",
                "wildside",
                "status",
                "preview",
            ],
        )
    ], "Helm status must use the provider-specific kube context"
    assert (
        "kubectl --context kind-wildside-preview --namespace wildside "
        "port-forward svc/preview-wildside 8088:80"
    ) in output, "kind status must print the port-forward command for the Helm service"


def test_print_logs_uses_configured_kube_context(
    monkeypatch: pytest.MonkeyPatch,
    preview_config: PreviewConfig,
) -> None:
    """Verify log streaming targets the provider-specific kube context."""
    config = replace(preview_config, k8s_provider="kind")
    commands: list[tuple[str, list[str]]] = []

    def record_run(command: str, args: list[str], **_: object) -> SimpleNamespace:
        commands.append((command, args))
        return SimpleNamespace(stdout="")

    monkeypatch.setattr("local_k8s.deployment.require_tools", lambda _: None)
    monkeypatch.setattr("local_k8s.deployment.run", record_run)

    print_logs(config, follow=True)

    assert commands == [
        (
            "kubectl",
            [
                "--context",
                "kind-wildside-preview",
                "-n",
                "wildside",
                "logs",
                "-l",
                "app.kubernetes.io/instance=preview",
                "-c",
                "app",
                "--tail",
                "200",
                "--follow",
            ],
        )
    ], "logs must use the provider-specific kube context"
