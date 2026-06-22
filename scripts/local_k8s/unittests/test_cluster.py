"""Unit tests for provider-aware local preview cluster lifecycle.

These tests exercise the command contracts used by the local preview cluster
adapter without creating real Kubernetes clusters. They cover the legacy k3d
path and its loopback load-balancer port mapping, Docker-backed kind cluster
creation using a stdin config with no host port mappings, and rootless
Podman-backed kind creation wrapped in a delegated user `systemd-run` scope.

Run this module with:

```
PYTHONPATH=scripts uv run --with pytest --with plumbum pytest scripts/local_k8s/unittests/test_cluster.py
```
"""

from __future__ import annotations

from dataclasses import dataclass, replace

import pytest

from local_k8s.cluster import ensure_cluster
from local_k8s.config import PreviewConfig


@dataclass(frozen=True, slots=True)
class MockCommandResult:
    """Minimal command result for cluster command-runner tests."""

    stdout: str = ""
    stderr: str = ""


class TestClusterCreation:
    """Provider command-contract tests for preview cluster creation."""

    def test_k3d_cluster_creation_keeps_loopback_load_balancer(
        self,
        monkeypatch: pytest.MonkeyPatch,
        preview_config: PreviewConfig,
    ) -> None:
        """Verify the default k3d path keeps its existing port mapping contract."""
        commands: list[tuple[str, list[str], str | None]] = []

        def record_run(command: str, args: list[str], **kwargs: object) -> MockCommandResult:
            commands.append((command, args, kwargs.get("input_text")))
            if args == ["cluster", "list", "--output", "json"]:
                return MockCommandResult(stdout="[]")
            return MockCommandResult()

        monkeypatch.setattr("local_k8s.cluster.require_tools", lambda _: None)
        monkeypatch.setattr("local_k8s.cluster.run", record_run)

        ensure_cluster(preview_config)

        assert commands == [
            ("k3d", ["cluster", "list", "--output", "json"], None),
            (
                "k3d",
                [
                    "cluster",
                    "create",
                    "wildside-preview",
                    "--servers",
                    "1",
                    "--agents",
                    "1",
                    "--port",
                    "127.0.0.1:8088:80@loadbalancer",
                    "--wait",
                ],
                None,
            ),
        ], "k3d cluster creation must keep 127.0.0.1:8088:80@loadbalancer"

    def test_kind_cluster_creation_uses_stdin_config_without_host_ports(
        self,
        monkeypatch: pytest.MonkeyPatch,
        preview_config: PreviewConfig,
    ) -> None:
        """Verify Docker-backed kind creation avoids host-port mappings."""
        config = replace(preview_config, k8s_provider="kind")
        commands: list[tuple[str, list[str], str | None]] = []

        def record_run(command: str, args: list[str], **kwargs: object) -> MockCommandResult:
            commands.append((command, args, kwargs.get("input_text")))
            if command == "kind" and args == ["get", "clusters"]:
                return MockCommandResult(stdout="other\n")
            return MockCommandResult()

        monkeypatch.setattr("local_k8s.cluster.require_tools", lambda _: None)
        monkeypatch.setattr("local_k8s.cluster.run", record_run)

        ensure_cluster(config)

        assert commands[0] == ("kind", ["get", "clusters"], None), "kind must check for existing clusters"
        assert commands[1][0:2] == (
            "kind",
            [
                "create",
                "cluster",
                "--name",
                "wildside-preview",
                "--config",
                "-",
                "--wait",
                "180s",
            ],
        ), "kind cluster creation must use stdin config via --config -"
        assert commands[1][2] is not None, "kind cluster creation must pass config on stdin"
        assert "kind: Cluster" in commands[1][2], "stdin config must be valid kind Cluster YAML"
        assert "extraPortMappings" not in commands[1][2], "Docker-backed kind must not use host port mappings"

    def test_podman_kind_cluster_creation_uses_rootless_scope(
        self,
        monkeypatch: pytest.MonkeyPatch,
        preview_config: PreviewConfig,
    ) -> None:
        """Verify rootless Podman kind creation runs in a delegated user scope."""
        config = replace(preview_config, container_engine="podman", k8s_provider="kind")
        commands: list[tuple[str, list[str], str | None]] = []

        def record_run(command: str, args: list[str], **kwargs: object) -> MockCommandResult:
            commands.append((command, args, kwargs.get("input_text")))
            if command == "env" and args == ["KIND_EXPERIMENTAL_PROVIDER=podman", "kind", "get", "clusters"]:
                return MockCommandResult()
            return MockCommandResult()

        monkeypatch.setattr("local_k8s.cluster.require_tools", lambda _: None)
        monkeypatch.setattr("local_k8s.cluster.run", record_run)

        ensure_cluster(config)

        assert commands[0] == (
            "env",
            ["KIND_EXPERIMENTAL_PROVIDER=podman", "kind", "get", "clusters"],
            None,
        ), "KIND_EXPERIMENTAL_PROVIDER must be set for Podman-backed kind"
        assert commands[1][0] == "systemd-run", "Podman-backed kind must be wrapped in systemd-run"
        assert commands[1][1][:7] == [
            "--scope",
            "--user",
            "-p",
            "Delegate=yes",
            "env",
            "KIND_EXPERIMENTAL_PROVIDER=podman",
            "kind",
        ], "systemd-run must use --user --scope with Delegate=yes for rootless Podman"
        assert commands[1][2] is not None, "Podman kind creation must pass config on stdin"
