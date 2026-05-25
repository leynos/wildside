"""Unit tests for local preview Kubernetes helpers."""

from __future__ import annotations

import typing
from pathlib import Path
from types import SimpleNamespace

from local_k8s.config import PreviewConfig
from local_k8s.k8s import print_kubernetes_status

if typing.TYPE_CHECKING:
    import pytest


def test_print_kubernetes_status_uses_helm_fullname_for_service(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Verify that status queries the Helm-derived Service name."""
    config = PreviewConfig(
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
    calls: list[tuple[str, list[str]]] = []

    def fake_run(command: str, args: list[str]) -> SimpleNamespace:
        calls.append((command, args))
        return SimpleNamespace(stdout="")

    monkeypatch.setattr("local_k8s.k8s.require_tools", lambda _: None)
    monkeypatch.setattr("local_k8s.k8s.run", fake_run)

    print_kubernetes_status(config)

    assert (
        "kubectl",
        ["-n", "wildside", "get", "service", "preview-wildside", "--ignore-not-found"],
    ) in calls, "expected kubectl get service preview-wildside call to be present in calls"
