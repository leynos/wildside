"""Unit tests for local preview status and log inspection."""

from __future__ import annotations

import dataclasses as dc
import typing as typ

from conftest import install_run_recorder
from local_k8s.deployment import print_logs, print_status

if typ.TYPE_CHECKING:
    import pytest
    from local_k8s.config import PreviewConfig


def test_print_status_uses_provider_context_and_prints_kind_port_forward(
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    preview_config: PreviewConfig,
) -> None:
    """Verify kind status uses provider tools and prints the operator command."""
    config = dc.replace(preview_config, k8s_provider="kind")
    required_tools: list[tuple[str, ...]] = []
    calls: list[str] = []
    commands = install_run_recorder(
        monkeypatch,
        stdout="helm status\n",
        on_run=lambda _command, _args, _input_text: calls.append("helm_status"),
    )

    def record_require_tools(tools: tuple[str, ...]) -> None:
        calls.append("require_tools")
        required_tools.append(tuple(tools))

    monkeypatch.setattr(
        "local_k8s.deployment.require_tools",
        record_require_tools,
    )
    monkeypatch.setattr(
        "local_k8s.deployment.print_cluster_status",
        lambda _: calls.append("print_cluster_status"),
    )
    monkeypatch.setattr(
        "local_k8s.deployment.print_kubernetes_status",
        lambda _: calls.append("print_kubernetes_status"),
    )

    print_status(config)

    output = capsys.readouterr().out
    assert required_tools == [("helm", "kind", "kubectl")], (
        "kind status must require the helm, kind, and kubectl provider tools"
    )
    assert calls == [
        "require_tools",
        "print_cluster_status",
        "helm_status",
        "print_kubernetes_status",
    ], (
        "status must require tools, then print cluster status, inspect the Helm "
        "release, and print Kubernetes status in that order"
    )
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
            None,
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
    config = dc.replace(preview_config, k8s_provider="kind")
    commands = install_run_recorder(monkeypatch)
    streaming_commands: list[tuple[str, list[str]]] = []

    def record_streaming(command: str, args: list[str]) -> None:
        streaming_commands.append((command, args))

    monkeypatch.setattr("local_k8s.deployment.require_tools", lambda _: None)
    monkeypatch.setattr("local_k8s.deployment.run_streaming", record_streaming)

    print_logs(config, follow=True)

    assert commands == [], "followed logs must stream rather than capture output"
    assert streaming_commands == [
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
