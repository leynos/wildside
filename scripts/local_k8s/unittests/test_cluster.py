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
        required_tools: list[tuple[str, ...]] = []
        commands: list[tuple[str, list[str], str | None]] = []

        def record_run(command: str, args: list[str], **kwargs: object) -> MockCommandResult:
            commands.append((command, args, kwargs.get("input_text")))
            if command == "env" and args == ["KIND_EXPERIMENTAL_PROVIDER=podman", "kind", "get", "clusters"]:
                return MockCommandResult()
            return MockCommandResult()

        monkeypatch.setattr(
            "local_k8s.cluster.require_tools",
            lambda tools: required_tools.append(tuple(tools)),
        )
        monkeypatch.setattr("local_k8s.cluster.run", record_run)

        ensure_cluster(config)

        assert required_tools == [("kind", "podman", "kubectl", "helm", "systemd-run")]
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
    def _archive_path_from_save(commands: list[tuple[str, list[str]]]) -> Path:
        """Return the archive path passed to ``podman save``."""
        save_commands = [
            args for command, args in commands if command == "podman" and args[:2] == ["save", "--output"]
        ]
        assert len(save_commands) == 1, "Podman import must save exactly one image archive"
        return Path(save_commands[0][2])

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
        commands = self._capture_commands(
            monkeypatch,
            preview_config_kind,
            archive_dir=tmp_path,
            on_remove_archive=removed_archives.append,
        )
        archive_path = self._archive_path_from_save(commands)

        assert archive_path.parent == tmp_path, "Podman image archives must use the configured directory"
        assert archive_path.name.startswith("wildside-preview-"), (
            "Podman image archives must include the cluster name for operator diagnostics"
        )
        assert archive_path.name.endswith("-image.tar"), "Podman image archives must keep a tar suffix"
        assert removed_archives == [archive_path], "Podman image archives must be removed after load"
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

    def test_podman_kind_image_import_removes_archive_when_load_fails(
        self,
        monkeypatch: pytest.MonkeyPatch,
        preview_config_kind: PreviewConfig,
        tmp_path: Path,
    ) -> None:
        """Verify failed kind loads do not leave Podman image archives behind."""
        removed_archives: list[Path] = []
        command_log: list[tuple[str, list[str]]] = []

        def record_run(command: str, args: list[str], **_: object) -> MockCommandResult:
            command_log.append((command, args))
            if command == "env" and args[:3] == ["KIND_EXPERIMENTAL_PROVIDER=podman", "kind", "load"]:
                raise LocalK8sError("kind load failed")
            return MockCommandResult()

        monkeypatch.setattr("local_k8s.cluster.require_tools", lambda _: None)
        monkeypatch.setattr("local_k8s.cluster.run", record_run)
        monkeypatch.setattr("local_k8s.cluster._remove_stale_archive", removed_archives.append)

        with pytest.raises(LocalK8sError, match="kind load failed"):
            import_image(preview_config_kind, archive_dir=tmp_path)

        archive_path = self._archive_path_from_save(command_log)
        assert removed_archives == [archive_path], "Podman image archives must be removed even when kind load fails"

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
        commands = self._capture_commands(monkeypatch, config, archive_dir=tmp_path)
        archive_path = self._archive_path_from_save(commands)

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
        commands = self._capture_commands(monkeypatch, config, archive_dir=tmp_path)
        archive_path = self._archive_path_from_save(commands)

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

    def test_podman_kind_image_import_uses_unique_archive_names(
        self,
        monkeypatch: pytest.MonkeyPatch,
        preview_config_kind: PreviewConfig,
        tmp_path: Path,
    ) -> None:
        """Verify duplicate imports do not contend for one archive path."""
        first_commands = self._capture_commands(monkeypatch, preview_config_kind, archive_dir=tmp_path)
        second_commands = self._capture_commands(monkeypatch, preview_config_kind, archive_dir=tmp_path)

        assert self._archive_path_from_save(first_commands) != self._archive_path_from_save(
            second_commands
        ), "Podman image imports must use unique archive paths"


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
    """Verify kind cluster status reports context without a direct ingress URL."""
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
    assert "ingress:" not in output
    assert "http://127.0.0.1:8088" not in output


def test_print_cluster_status_prints_k3d_ingress(
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    preview_config: PreviewConfig,
) -> None:
    """Verify k3d status keeps the direct ingress URL."""

    def record_run(_command: str, _args: list[str], **_: object) -> MockCommandResult:
        return MockCommandResult(stdout='[{"name":"wildside-preview"}]\n')

    monkeypatch.setattr("local_k8s.cluster.require_tools", lambda _: None)
    monkeypatch.setattr("local_k8s.cluster.run", record_run)

    print_cluster_status(preview_config)

    output = capsys.readouterr().out
    assert "provider: k3d" in output
    assert "ingress: http://127.0.0.1:8088" in output
    assert "port-forward address:" not in output


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
