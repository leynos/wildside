"""Unit tests for local preview image import."""

from __future__ import annotations

from collections.abc import Callable
from dataclasses import dataclass, replace
from pathlib import Path

import pytest
import pytest_mock

from local_k8s.cluster import import_image
from local_k8s.config import PreviewConfig
from local_k8s.validation import LocalK8sError


@dataclass(frozen=True, slots=True)
class MockCommandResult:
    """Minimal command result for image-import command-runner tests."""

    stdout: str = ""
    stderr: str = ""


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
        mocker: pytest_mock.MockerFixture,
        config: PreviewConfig,
        *,
        archive_dir: Path | None = None,
        on_remove_archive: Callable[[Path], None] | None = None,
    ) -> list[tuple[str, list[str]]]:
        """Patch cluster internals, run import_image, and return recorded commands."""
        commands: list[tuple[str, list[str]]] = []

        def record_run(command: str, args: list[str], **_: object) -> MockCommandResult:
            commands.append((command, args))
            return MockCommandResult()

        mocker.patch("local_k8s.cluster.require_tools", return_value=None)
        mocker.patch("local_k8s.cluster.run", record_run)
        if archive_dir is not None:
            if on_remove_archive is not None:
                mocker.patch(
                    "local_k8s.cluster._remove_stale_archive",
                    on_remove_archive,
                )
            else:
                mocker.patch(
                    "local_k8s.cluster._remove_stale_archive",
                    return_value=None,
                )

        import_image(config, archive_dir=archive_dir)
        return commands

    def test_k3d_image_import_uses_existing_provider_command(
        self,
        mocker: pytest_mock.MockerFixture,
        preview_config: PreviewConfig,
    ) -> None:
        """Verify k3d keeps its current image import command."""
        commands = self._capture_commands(mocker, preview_config)

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
        mocker: pytest_mock.MockerFixture,
        preview_config: PreviewConfig,
    ) -> None:
        """Verify Docker-backed kind loads the local Docker image directly."""
        config = replace(preview_config, k8s_provider="kind")
        commands = self._capture_commands(mocker, config)

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
        mocker: pytest_mock.MockerFixture,
        preview_config_kind: PreviewConfig,
        tmp_path: Path,
    ) -> None:
        """Verify rootless Podman kind uses an archive load path."""
        removed_archives: list[Path] = []
        commands = self._capture_commands(
            mocker,
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
        mocker: pytest_mock.MockerFixture,
        preview_config_kind: PreviewConfig,
        tmp_path: Path,
    ) -> None:
        """Verify failed kind loads do not leave Podman image archives behind."""
        removed_archives: list[Path] = []
        command_log: list[tuple[str, list[str]]] = []

        def record_run(command: str, args: list[str], **_: object) -> MockCommandResult:
            command_log.append((command, args))
            if command == "env" and args[:3] == ["KIND_EXPERIMENTAL_PROVIDER=podman", "kind", "load"]:
                error_message = "kind load failed"
                raise LocalK8sError(error_message)
            return MockCommandResult()

        mocker.patch("local_k8s.cluster.require_tools", return_value=None)
        mocker.patch("local_k8s.cluster.run", record_run)
        mocker.patch("local_k8s.cluster._remove_stale_archive", removed_archives.append)

        with pytest.raises(LocalK8sError, match="kind load failed"):
            import_image(preview_config_kind, archive_dir=tmp_path)

        archive_path = self._archive_path_from_save(command_log)
        assert removed_archives == [archive_path], "Podman image archives must be removed even when kind load fails"

    def test_podman_kind_image_import_keeps_registry_qualified_archive_tag(
        self,
        mocker: pytest_mock.MockerFixture,
        preview_config_kind: PreviewConfig,
        tmp_path: Path,
    ) -> None:
        """Verify registry-qualified image names are archived without retagging."""
        config = replace(
            preview_config_kind,
            image_name="registry.example.test/wildside/backend:local",
        )
        commands = self._capture_commands(mocker, config, archive_dir=tmp_path)
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
        mocker: pytest_mock.MockerFixture,
        preview_config_kind: PreviewConfig,
        tmp_path: Path,
    ) -> None:
        """Verify namespaced Docker Hub image names are archived as Kubernetes pulls them."""
        config = replace(
            preview_config_kind,
            image_name="leynos/wildside-backend:local",
        )
        commands = self._capture_commands(mocker, config, archive_dir=tmp_path)
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
        mocker: pytest_mock.MockerFixture,
        preview_config_kind: PreviewConfig,
        tmp_path: Path,
    ) -> None:
        """Verify duplicate imports do not contend for one archive path."""
        first_commands = self._capture_commands(mocker, preview_config_kind, archive_dir=tmp_path)
        second_commands = self._capture_commands(mocker, preview_config_kind, archive_dir=tmp_path)

        assert self._archive_path_from_save(first_commands) != self._archive_path_from_save(
            second_commands
        ), "Podman image imports must use unique archive paths"
