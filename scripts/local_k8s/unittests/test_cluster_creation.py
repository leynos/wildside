"""Unit tests for local preview cluster creation."""

from __future__ import annotations

import dataclasses as dc
import typing as typ

from local_k8s.cluster import ensure_cluster

if typ.TYPE_CHECKING:
    import collections.abc as cabc

    import pytest
    from local_k8s.config import PreviewConfig


@dc.dataclass(frozen=True, slots=True)
class MockCommandResult:
    """Minimal command result for cluster command-runner tests."""

    stdout: str = ""
    stderr: str = ""


type CommandRecord = tuple[str, list[str], str | None]


def install_cluster_run_recorder(
    monkeypatch: pytest.MonkeyPatch,
    responder: cabc.Callable[[str, list[str], str | None], MockCommandResult],
    *,
    on_require_tools: cabc.Callable[[tuple[str, ...]], None] | None = None,
) -> list[CommandRecord]:
    """Install cluster command recording for provider contract tests."""
    commands: list[CommandRecord] = []

    def record_run(
        command: str,
        args: list[str],
        *,
        input_text: str | None = None,
    ) -> MockCommandResult:
        commands.append((command, args, input_text))
        return responder(command, args, input_text)

    def record_require_tools(tools: tuple[str, ...]) -> None:
        if on_require_tools is not None:
            on_require_tools(tuple(tools))

    monkeypatch.setattr("local_k8s.cluster.require_tools", record_require_tools)
    monkeypatch.setattr("local_k8s.cluster.run", record_run)
    return commands


class TestClusterCreation:
    """Provider command-contract tests for preview cluster creation."""

    def test_k3d_cluster_creation_keeps_loopback_load_balancer(
        self,
        monkeypatch: pytest.MonkeyPatch,
        preview_config: PreviewConfig,
    ) -> None:
        """Verify the default k3d path keeps its existing port mapping contract."""

        def respond(
            _command: str, args: list[str], _input_text: str | None
        ) -> MockCommandResult:
            if args == ["cluster", "list", "--output", "json"]:
                return MockCommandResult(stdout="[]")
            return MockCommandResult()

        commands = install_cluster_run_recorder(monkeypatch, respond)

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

    def test_ensure_cluster_skips_creation_when_cluster_exists(
        self,
        monkeypatch: pytest.MonkeyPatch,
        preview_config: PreviewConfig,
        capsys: pytest.CaptureFixture[str],
    ) -> None:
        """An already-present cluster is reported and never recreated."""

        def respond(
            _command: str, args: list[str], _input_text: str | None
        ) -> MockCommandResult:
            if args == ["cluster", "list", "--output", "json"]:
                return MockCommandResult(stdout='[{"name": "wildside-preview"}]')
            return MockCommandResult()

        commands = install_cluster_run_recorder(monkeypatch, respond)

        ensure_cluster(preview_config)

        assert commands == [
            ("k3d", ["cluster", "list", "--output", "json"], None),
        ], "an existing cluster must be detected without issuing a create command"
        assert "already exists" in capsys.readouterr().out

    def test_kind_cluster_creation_uses_stdin_config_without_host_ports(
        self,
        monkeypatch: pytest.MonkeyPatch,
        preview_config: PreviewConfig,
    ) -> None:
        """Verify Docker-backed kind creation avoids host-port mappings."""
        config = dc.replace(preview_config, k8s_provider="kind")

        def respond(
            command: str, args: list[str], _input_text: str | None
        ) -> MockCommandResult:
            if command == "kind" and args == ["get", "clusters"]:
                return MockCommandResult(stdout="other\n")
            return MockCommandResult()

        commands = install_cluster_run_recorder(monkeypatch, respond)

        ensure_cluster(config)

        assert commands[0] == ("kind", ["get", "clusters"], None), (
            "kind must check for existing clusters"
        )
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
        assert commands[1][2] is not None, (
            "kind cluster creation must pass config on stdin"
        )
        assert "kind: Cluster" in commands[1][2], (
            "stdin config must be valid kind Cluster YAML"
        )
        assert 'image: "kindest/node:v1.31.0"' in commands[1][2], (
            "kind cluster creation must pin a Kubernetes version compatible "
            "with the Helm chart kubeVersion range"
        )
        assert "extraPortMappings" not in commands[1][2], (
            "Docker-backed kind must not use host port mappings"
        )

    def test_podman_kind_cluster_creation_uses_rootless_scope(
        self,
        monkeypatch: pytest.MonkeyPatch,
        preview_config: PreviewConfig,
    ) -> None:
        """Verify rootless Podman kind creation runs in a delegated user scope."""
        config = dc.replace(
            preview_config, container_engine="podman", k8s_provider="kind"
        )
        required_tools: list[tuple[str, ...]] = []

        def respond(
            _command: str, _args: list[str], _input_text: str | None
        ) -> MockCommandResult:
            return MockCommandResult()

        commands = install_cluster_run_recorder(
            monkeypatch,
            respond,
            on_require_tools=required_tools.append,
        )

        ensure_cluster(config)

        assert required_tools == [
            ("kind", "podman", "kubectl", "helm", "systemd-run")
        ], (
            "Podman-backed kind must require kind, podman, kubectl, helm, "
            "and systemd-run"
        )
        assert commands[0] == (
            "env",
            ["KIND_EXPERIMENTAL_PROVIDER=podman", "kind", "get", "clusters"],
            None,
        ), "KIND_EXPERIMENTAL_PROVIDER must be set for Podman-backed kind"
        assert commands[1][:2] == (
            "systemd-run",
            [
                "--scope",
                "--user",
                "-p",
                "Delegate=yes",
                "env",
                "KIND_EXPERIMENTAL_PROVIDER=podman",
                "kind",
                "create",
                "cluster",
                "--name",
                "wildside-preview",
                "--config",
                "-",
                "--wait",
                "180s",
            ],
        ), (
            "systemd-run must wrap the full kind create invocation in a "
            "--user --scope with Delegate=yes for rootless Podman"
        )
        assert commands[1][2] is not None, (
            "Podman kind creation must pass config on stdin"
        )
        assert "kind: Cluster" in commands[1][2], (
            "stdin config must be valid kind Cluster YAML"
        )
        assert 'image: "kindest/node:v1.31.0"' in commands[1][2], (
            "Podman kind creation must pin a Kubernetes version compatible "
            "with the Helm chart kubeVersion range"
        )
        assert "extraPortMappings" not in commands[1][2], (
            "Podman-backed kind must rely on port-forwarding rather than "
            "host port mappings"
        )
