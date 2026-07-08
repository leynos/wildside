"""Unit tests for local preview cluster status and deletion."""

from __future__ import annotations

from dataclasses import replace

import pytest

from local_k8s.cluster import delete_cluster, print_cluster_status
from local_k8s.commands import CommandResult
from local_k8s.config import PreviewConfig
from local_k8s.validation import LocalK8sError


class TestClusterStatus:
    """Provider command-contract tests for cluster status and deletion."""

    def test_kind_delete_is_idempotent_when_cluster_is_absent(
        self,
        monkeypatch: pytest.MonkeyPatch,
        preview_config: PreviewConfig,
    ) -> None:
        """Verify kind teardown skips deletion when the cluster is absent."""
        config = replace(preview_config, k8s_provider="kind")
        commands: list[tuple[str, list[str]]] = []

        def record_run(command: str, args: list[str], **_: object) -> CommandResult:
            commands.append((command, args))
            return CommandResult(stdout="other\n", stderr="")

        monkeypatch.setattr("local_k8s.cluster.require_tools", lambda _: None)
        monkeypatch.setattr("local_k8s.cluster.run", record_run)

        delete_cluster(config)

        assert commands == [
            ("kind", ["get", "clusters"]),
        ], "kind down must not delete an absent cluster"

    def test_print_cluster_status_prints_provider_context(
        self,
        monkeypatch: pytest.MonkeyPatch,
        capsys: pytest.CaptureFixture[str],
        preview_config: PreviewConfig,
    ) -> None:
        """Verify kind cluster status reports context without a direct ingress URL."""
        commands: list[tuple[str, list[str]]] = []

        def record_run(command: str, args: list[str], **_: object) -> CommandResult:
            commands.append((command, args))
            return CommandResult(stdout="wildside-preview\n", stderr="")

        monkeypatch.setattr("local_k8s.cluster.require_tools", lambda _: None)
        monkeypatch.setattr("local_k8s.cluster.run", record_run)

        print_cluster_status(replace(preview_config, k8s_provider="kind"))

        output = capsys.readouterr().out
        assert commands == [("kind", ["get", "clusters"])]
        assert "cluster: wildside-preview" in output
        assert "provider: kind" in output
        assert "ingress:" not in output
        assert "http://127.0.0.1:8088" not in output

    def test_print_cluster_status_prints_k3d_ingress(
        self,
        monkeypatch: pytest.MonkeyPatch,
        capsys: pytest.CaptureFixture[str],
        preview_config: PreviewConfig,
    ) -> None:
        """Verify k3d status keeps the direct ingress URL."""

        def record_run(_command: str, _args: list[str], **_: object) -> CommandResult:
            return CommandResult(stdout='[{"name":"wildside-preview"}]\n', stderr="")

        monkeypatch.setattr("local_k8s.cluster.require_tools", lambda _: None)
        monkeypatch.setattr("local_k8s.cluster.run", record_run)

        print_cluster_status(preview_config)

        output = capsys.readouterr().out
        assert "provider: k3d" in output
        assert "ingress: http://127.0.0.1:8088" in output
        assert "port-forward address:" not in output

    def test_print_cluster_status_rejects_missing_cluster(
        self,
        monkeypatch: pytest.MonkeyPatch,
        preview_config: PreviewConfig,
    ) -> None:
        """Verify cluster status fails before printing stale preview details."""
        monkeypatch.setattr("local_k8s.cluster.require_tools", lambda _: None)
        monkeypatch.setattr(
            "local_k8s.cluster.run",
            lambda *_args, **_kwargs: CommandResult(stdout="other\n", stderr=""),
        )

        with pytest.raises(LocalK8sError, match="does not exist"):
            print_cluster_status(replace(preview_config, k8s_provider="kind"))

    def test_k3d_cluster_exists_rejects_malformed_json(
        self,
        monkeypatch: pytest.MonkeyPatch,
        preview_config: PreviewConfig,
    ) -> None:
        """Verify k3d status fails when `cluster list` emits non-JSON output."""
        monkeypatch.setattr("local_k8s.cluster.require_tools", lambda _: None)
        monkeypatch.setattr(
            "local_k8s.cluster.run",
            lambda *_args, **_kwargs: CommandResult(stdout="not json", stderr=""),
        )

        with pytest.raises(
            LocalK8sError, match="unexpected k3d cluster list JSON payload"
        ):
            print_cluster_status(preview_config)

    def test_k3d_cluster_exists_rejects_non_list_payload(
        self,
        monkeypatch: pytest.MonkeyPatch,
        preview_config: PreviewConfig,
    ) -> None:
        """Verify k3d status fails when `cluster list` JSON is not a list."""
        monkeypatch.setattr("local_k8s.cluster.require_tools", lambda _: None)
        monkeypatch.setattr(
            "local_k8s.cluster.run",
            lambda *_args, **_kwargs: CommandResult(
                stdout='{"name": "wildside-preview"}', stderr=""
            ),
        )

        with pytest.raises(
            LocalK8sError, match="unexpected k3d cluster list JSON shape"
        ):
            print_cluster_status(preview_config)
