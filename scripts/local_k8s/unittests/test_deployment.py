"""Unit tests for local preview deployment orchestration.

These tests exercise the orchestration logic in ``local_k8s.deployment``
without invoking Kubernetes, Helm, k3d, or Docker. They document the preflight
contract for full build-and-deploy runs and the ``skip_build`` path used with
prebuilt images. The key invariant is that Docker is required only when the
deployment will build an image locally; Helm, k3d, and kubectl remain required
for both deployment modes.
"""

from __future__ import annotations

from pathlib import Path

import pytest

from local_k8s.config import PreviewConfig
from local_k8s.deployment import deploy_preview


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
        cluster_name="wildside-preview",
        namespace="wildside",
        release_name="preview",
        image_name="wildside-backend:local",
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
    monkeypatch.setattr("local_k8s.deployment.import_image", no_op)
    monkeypatch.setattr("local_k8s.deployment.helm_upgrade", no_op)
    monkeypatch.setattr("local_k8s.deployment.print_status", no_op)
    monkeypatch.setattr("local_k8s.deployment.build_image", no_op)

    deploy_preview(preview_config, skip_build=skip_build)

    assert required_tools == [expected_tools], (
        f"expected require_tools to be called once with {expected_tools}, "
        f"but got {required_tools}"
    )
