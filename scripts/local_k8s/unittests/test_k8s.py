"""Unit tests for local preview Kubernetes helpers."""

from __future__ import annotations

import typing
from dataclasses import replace
from types import SimpleNamespace

from local_k8s.config import PreviewConfig
from local_k8s.k8s import print_kubernetes_status

if typing.TYPE_CHECKING:
    import pytest


def _capture_status_commands(monkeypatch: pytest.MonkeyPatch) -> list[tuple[str, list[str]]]:
    """Install status command stubs and return captured kubectl calls."""
    calls: list[tuple[str, list[str]]] = []

    def fake_run(command: str, args: list[str]) -> SimpleNamespace:
        calls.append((command, args))
        return SimpleNamespace(stdout="")

    monkeypatch.setattr("local_k8s.k8s.require_tools", lambda _: None)
    monkeypatch.setattr("local_k8s.k8s.run", fake_run)
    return calls


def test_print_kubernetes_status_uses_helm_fullname_for_service(
    monkeypatch: pytest.MonkeyPatch,
    preview_config: PreviewConfig,
) -> None:
    """Verify that status queries the Helm-derived Service name."""
    config = preview_config
    calls = _capture_status_commands(monkeypatch)

    print_kubernetes_status(config)

    assert (
        "kubectl",
        [
            "--context",
            "k3d-wildside-preview",
            "-n",
            "wildside",
            "get",
            "service",
            "preview-wildside",
            "--ignore-not-found",
        ],
    ) in calls, "expected kubectl get service preview-wildside call to be present in calls"


def test_print_kubernetes_status_uses_configured_kube_context(
    monkeypatch: pytest.MonkeyPatch,
    preview_config: PreviewConfig,
) -> None:
    """Verify status uses the kube context for the selected provider."""
    config = replace(preview_config, k8s_provider="kind")
    calls = _capture_status_commands(monkeypatch)

    print_kubernetes_status(config)

    assert calls[0] == (
        "kubectl",
        [
            "--context",
            "kind-wildside-preview",
            "-n",
            "wildside",
            "get",
            "pods",
            "-l",
            "app.kubernetes.io/instance=preview",
            "-o",
            "wide",
        ],
    ), "kind status must use the kind-prefixed kube context"
