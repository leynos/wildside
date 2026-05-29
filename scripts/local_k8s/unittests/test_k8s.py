"""Unit tests for local preview Kubernetes helpers."""

from __future__ import annotations

import typing
from types import SimpleNamespace

from local_k8s.config import PreviewConfig
from local_k8s.k8s import print_kubernetes_status

if typing.TYPE_CHECKING:
    import pytest


def test_print_kubernetes_status_uses_helm_fullname_for_service(
    monkeypatch: pytest.MonkeyPatch,
    preview_config: PreviewConfig,
) -> None:
    """Verify that status queries the Helm-derived Service name."""
    config = preview_config
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
