"""Unit tests for local preview cluster status and deletion."""

from __future__ import annotations

import dataclasses as dc
import typing as typ

import pytest
from local_k8s.cluster import delete_cluster, print_cluster_status
from local_k8s.commands import CommandResult
from local_k8s.validation import LocalK8sError

if typ.TYPE_CHECKING:
    import pytest_mock
    from local_k8s.config import PreviewConfig


class TestClusterStatus:
    """Provider command-contract tests for cluster status and deletion."""

    @staticmethod
    def _assert_cluster_status_rejects(
        mocker: pytest_mock.MockerFixture,
        config: PreviewConfig,
        capsys: pytest.CaptureFixture[str],
        *,
        stdout: str,
        match: str,
    ) -> None:
        """Assert ``print_cluster_status`` raises a matching error.

        Stubs ``run()`` with the given stdout payload first.
        """
        mocker.patch("local_k8s.cluster.require_tools", return_value=None)
        mocker.patch(
            "local_k8s.cluster.run",
            return_value=CommandResult(stdout=stdout, stderr=""),
        )

        with pytest.raises(LocalK8sError, match=match):
            print_cluster_status(config)

        assert capsys.readouterr().out == "", (
            "status must not print stale details before rejecting"
        )

    def test_kind_delete_is_idempotent_when_cluster_is_absent(
        self,
        mocker: pytest_mock.MockerFixture,
        preview_config: PreviewConfig,
    ) -> None:
        """Verify kind teardown skips deletion when the cluster is absent."""
        config = dc.replace(preview_config, k8s_provider="kind")
        commands: list[tuple[str, list[str]]] = []

        def record_run(command: str, args: list[str], **_: object) -> CommandResult:
            commands.append((command, args))
            return CommandResult(stdout="other\n", stderr="")

        mocker.patch("local_k8s.cluster.require_tools", return_value=None)
        mocker.patch("local_k8s.cluster.run", record_run)

        delete_cluster(config)

        assert commands == [
            ("kind", ["get", "clusters"]),
        ], "kind down must not delete an absent cluster"

    def test_print_cluster_status_prints_provider_context(
        self,
        mocker: pytest_mock.MockerFixture,
        capsys: pytest.CaptureFixture[str],
        preview_config: PreviewConfig,
    ) -> None:
        """Verify kind cluster status reports context without a direct ingress URL."""
        commands: list[tuple[str, list[str]]] = []

        def record_run(command: str, args: list[str], **_: object) -> CommandResult:
            commands.append((command, args))
            return CommandResult(stdout="wildside-preview\n", stderr="")

        mocker.patch("local_k8s.cluster.require_tools", return_value=None)
        mocker.patch("local_k8s.cluster.run", record_run)

        print_cluster_status(dc.replace(preview_config, k8s_provider="kind"))

        output = capsys.readouterr().out
        assert commands == [("kind", ["get", "clusters"])]
        assert "cluster: wildside-preview" in output
        assert "provider: kind" in output
        assert "ingress:" not in output
        assert "http://127.0.0.1:8088" not in output

    def test_print_cluster_status_prints_k3d_ingress(
        self,
        mocker: pytest_mock.MockerFixture,
        capsys: pytest.CaptureFixture[str],
        preview_config: PreviewConfig,
    ) -> None:
        """Verify k3d status keeps the direct ingress URL."""

        def record_run(_command: str, _args: list[str], **_: object) -> CommandResult:
            return CommandResult(stdout='[{"name":"wildside-preview"}]\n', stderr="")

        mocker.patch("local_k8s.cluster.require_tools", return_value=None)
        mocker.patch("local_k8s.cluster.run", record_run)

        print_cluster_status(preview_config)

        output = capsys.readouterr().out
        assert "provider: k3d" in output
        assert "ingress: http://127.0.0.1:8088" in output
        assert "port-forward address:" not in output

    def test_print_cluster_status_rejects_missing_cluster(
        self,
        mocker: pytest_mock.MockerFixture,
        capsys: pytest.CaptureFixture[str],
        preview_config: PreviewConfig,
    ) -> None:
        """Verify cluster status fails before printing stale preview details."""
        self._assert_cluster_status_rejects(
            mocker,
            dc.replace(preview_config, k8s_provider="kind"),
            capsys,
            stdout="other\n",
            match="does not exist",
        )

    @pytest.mark.parametrize(
        ("stdout", "match"),
        [
            pytest.param(
                "not json",
                "unexpected k3d cluster list JSON payload",
                id="malformed_json",
            ),
            pytest.param(
                '{"name": "wildside-preview"}',
                "unexpected k3d cluster list JSON shape",
                id="non_list_payload",
            ),
            pytest.param(
                "[{}]",
                "unexpected k3d cluster list entry",
                id="malformed_entry",
            ),
        ],
    )
    def test_k3d_cluster_exists_rejects_invalid_payload(
        self,
        mocker: pytest_mock.MockerFixture,
        capsys: pytest.CaptureFixture[str],
        preview_config: PreviewConfig,
        stdout: str,
        match: str,
    ) -> None:
        """Verify k3d status fails on malformed `cluster list` output."""
        self._assert_cluster_status_rejects(
            mocker, preview_config, capsys, stdout=stdout, match=match
        )
