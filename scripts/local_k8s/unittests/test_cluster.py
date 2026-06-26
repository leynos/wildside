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

from collections.abc import Callable
from dataclasses import dataclass, replace
from pathlib import Path

import pytest

from local_k8s.cluster import delete_cluster, ensure_cluster, import_image, print_cluster_status
from local_k8s.config import PreviewConfig
from local_k8s.validation import LocalK8sError


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
        assert 'image: "kindest/node:v1.31.0"' in commands[1][2], (
            "kind cluster creation must pin a Kubernetes version compatible "
            "with the Helm chart kubeVersion range"
        )
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


class TestImageImport:
    """Provider command-contract tests for preview image loading."""

    @staticmethod
    def _capture_commands(
        monkeypatch: pytest.MonkeyPatch,
        config: PreviewConfig,
        *,
        archive_dir: Path | None = None,
        on_remove_archive: Callable[[Path], None] | None = None,
    ) -> list[tuple[str, list[str]]]:
        """Monkeypatch cluster internals, run import_image, and return recorded commands."""
        commands: list[tuple[str, list[str]]] = []

        def record_run(command: str, args: list[str], **_: object) -> MockCommandResult:
            commands.append((command, args))
            return MockCommandResult()

        monkeypatch.setattr("local_k8s.cluster.require_tools", lambda _: None)
        monkeypatch.setattr("local_k8s.cluster.run", record_run)
        if archive_dir is not None:
            monkeypatch.setattr(
                "local_k8s.cluster._remove_stale_archive",
                on_remove_archive if on_remove_archive is not None else lambda _: None,
            )

        import_image(config, archive_dir=archive_dir)
        return commands

    def test_k3d_image_import_uses_existing_provider_command(
        self,
        monkeypatch: pytest.MonkeyPatch,
        preview_config: PreviewConfig,
    ) -> None:
        """Verify k3d keeps its current image import command."""
        commands = self._capture_commands(monkeypatch, preview_config)

        assert commands == [
            (
                "k3d",
                [
                    "image",
                    "import",
                    "wildside-backend:local",
                    "--cluster",
                    "wildside-preview",
                ],
            ),
        ], "k3d image import must remain unchanged"

    def test_docker_kind_image_import_uses_docker_image_loader(
        self,
        monkeypatch: pytest.MonkeyPatch,
        preview_config: PreviewConfig,
    ) -> None:
        """Verify Docker-backed kind loads the local Docker image directly."""
        config = replace(preview_config, k8s_provider="kind")
        commands = self._capture_commands(monkeypatch, config)

        assert commands == [
            (
                "kind",
                [
                    "load",
                    "docker-image",
                    "wildside-backend:local",
                    "--name",
                    "wildside-preview",
                ],
            ),
        ], "Docker-backed kind must use kind load docker-image"

    def test_podman_kind_image_import_saves_archive_before_loading(
        self,
        monkeypatch: pytest.MonkeyPatch,
        preview_config_kind: PreviewConfig,
        tmp_path: Path,
    ) -> None:
        """Verify rootless Podman kind uses an archive load path."""
        removed_archives: list[Path] = []
        archive_path = tmp_path / "wildside-preview-image.tar"
        commands = self._capture_commands(
            monkeypatch,
            preview_config_kind,
            archive_dir=tmp_path,
            on_remove_archive=removed_archives.append,
        )

        assert removed_archives == [archive_path], "stale Podman image archives must be removed before save"
        assert commands == [
            (
                "podman",
                [
                    "tag",
                    "wildside-backend:local",
                    "docker.io/library/wildside-backend:local",
                ],
            ),
            (
                "podman",
                [
                    "save",
                    "--output",
                    str(archive_path),
                    "docker.io/library/wildside-backend:local",
                ],
            ),
            (
                "env",
                [
                    "KIND_EXPERIMENTAL_PROVIDER=podman",
                    "kind",
                    "load",
                    "image-archive",
                    str(archive_path),
                    "--name",
                    "wildside-preview",
                ],
            ),
        ], "Podman-backed kind must archive the image name Kubernetes will pull"

    def test_podman_kind_image_import_keeps_registry_qualified_archive_tag(
        self,
        monkeypatch: pytest.MonkeyPatch,
        preview_config_kind: PreviewConfig,
        tmp_path: Path,
    ) -> None:
        """Verify registry-qualified image names are archived without retagging."""
        config = replace(
            preview_config_kind,
            image_name="registry.example.test/wildside/backend:local",
        )
        archive_path = tmp_path / "wildside-preview-image.tar"
        commands = self._capture_commands(monkeypatch, config, archive_dir=tmp_path)

        assert commands[0] == (
            "podman",
            [
                "save",
                "--output",
                str(archive_path),
                "registry.example.test/wildside/backend:local",
            ],
        ), "registry-qualified Podman images must not be retagged"

    def test_podman_kind_image_import_normalizes_namespaced_archive_tag(
        self,
        monkeypatch: pytest.MonkeyPatch,
        preview_config_kind: PreviewConfig,
        tmp_path: Path,
    ) -> None:
        """Verify namespaced Docker Hub image names are archived as Kubernetes pulls them."""
        config = replace(
            preview_config_kind,
            image_name="leynos/wildside-backend:local",
        )
        archive_path = tmp_path / "wildside-preview-image.tar"
        commands = self._capture_commands(monkeypatch, config, archive_dir=tmp_path)

        assert commands[:2] == [
            (
                "podman",
                [
                    "tag",
                    "leynos/wildside-backend:local",
                    "docker.io/leynos/wildside-backend:local",
                ],
            ),
            (
                "podman",
                [
                    "save",
                    "--output",
                    str(archive_path),
                    "docker.io/leynos/wildside-backend:local",
                ],
            ),
        ], "namespaced Podman images must use Kubernetes' Docker Hub pull name"


def test_kind_delete_is_idempotent_when_cluster_is_absent(
    monkeypatch: pytest.MonkeyPatch,
    preview_config: PreviewConfig,
) -> None:
    """Verify kind teardown skips deletion when the cluster is absent."""
    config = replace(preview_config, k8s_provider="kind")
    commands: list[tuple[str, list[str]]] = []

    def record_run(command: str, args: list[str], **_: object) -> MockCommandResult:
        commands.append((command, args))
        return MockCommandResult(stdout="other\n")

    monkeypatch.setattr("local_k8s.cluster.require_tools", lambda _: None)
    monkeypatch.setattr("local_k8s.cluster.run", record_run)

    delete_cluster(config)

    assert commands == [
        ("kind", ["get", "clusters"]),
    ], "kind down must not delete an absent cluster"


def test_print_cluster_status_prints_provider_context(
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    preview_config: PreviewConfig,
) -> None:
    """Verify cluster status reports the selected provider and ingress."""
    commands: list[tuple[str, list[str]]] = []

    def record_run(command: str, args: list[str], **_: object) -> MockCommandResult:
        commands.append((command, args))
        return MockCommandResult(stdout="wildside-preview\n")

    monkeypatch.setattr("local_k8s.cluster.require_tools", lambda _: None)
    monkeypatch.setattr("local_k8s.cluster.run", record_run)

    print_cluster_status(replace(preview_config, k8s_provider="kind"))

    output = capsys.readouterr().out
    assert commands == [("kind", ["get", "clusters"])]
    assert "cluster: wildside-preview" in output
    assert "provider: kind" in output
    assert "ingress: http://127.0.0.1:8088" in output


def test_print_cluster_status_rejects_missing_cluster(
    monkeypatch: pytest.MonkeyPatch,
    preview_config: PreviewConfig,
) -> None:
    """Verify cluster status fails before printing stale preview details."""
    monkeypatch.setattr("local_k8s.cluster.require_tools", lambda _: None)
    monkeypatch.setattr(
        "local_k8s.cluster.run",
        lambda *_args, **_kwargs: MockCommandResult(stdout="other\n"),
    )

    with pytest.raises(LocalK8sError, match="does not exist"):
        print_cluster_status(replace(preview_config, k8s_provider="kind"))
