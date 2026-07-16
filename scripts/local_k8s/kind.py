"""Command contracts for kind-backed local preview clusters.

This module builds the argument vectors and configuration payloads used to
create kind clusters and load images into them, supporting both Docker- and
rootless Podman-backed development environments. It also normalizes local image
names so Podman archives match the unqualified pull names Kubernetes expects.

The helpers here are pure command builders: they translate a resolved
:class:`local_k8s.config.PreviewConfig` into the executable, arguments, and
input text that :mod:`local_k8s.cluster` runs.
"""

from __future__ import annotations

import json
import logging
import shutil
import tempfile
from pathlib import Path
from types import TracebackType

from .config import PreviewConfig

logger = logging.getLogger(__name__)


def _kind_create_args(config: PreviewConfig) -> list[str]:
    """Return kind cluster creation arguments."""

    return [
        "create",
        "cluster",
        "--name",
        config.cluster_name,
        "--config",
        "-",
        "--wait",
        "180s",
    ]


def _kind_cluster_config(config: PreviewConfig) -> str:
    """Return a minimal kind cluster config with no host-port mapping."""

    node_image = json.dumps(config.kind_node_image)
    return f"""\
kind: Cluster
apiVersion: kind.x-k8s.io/v1alpha4
nodes:
  - role: control-plane
    image: {node_image}
"""


def _create_private_archive_dir(
    config: PreviewConfig, *, parent_dir: Path | None = None
) -> Path:
    """Create and return a fresh private directory for a Podman image archive.

    Side effect: creates a per-import directory via :func:`tempfile.mkdtemp`,
    owned by the current process with ``0700`` permissions, under ``parent_dir``
    (the system temporary directory by default). Because the directory is
    freshly created and private, no other user can pre-plant a symlink at the
    archive path within it, closing the time-of-check/time-of-use window that a
    bare reserved path would leave open. Returns the created directory.
    """

    base_dir = Path(tempfile.gettempdir()) if parent_dir is None else parent_dir
    return Path(tempfile.mkdtemp(prefix=f"{config.cluster_name}-", dir=base_dir))


def _image_archive_path(archive_dir: Path) -> Path:
    """Return the Podman image archive path within ``archive_dir``.

    Pure path computation with no filesystem side effects. ``archive_dir`` is
    the private directory produced by :func:`_create_private_archive_dir`.
    """

    return archive_dir / "image.tar"


def _is_registry_host(component: str) -> bool:
    """Return True when a repository path component identifies a registry host.

    A component is treated as a registry host when it contains a dot (FQDN),
    a colon (host:port), or is the literal string ``localhost``.
    """
    return "." in component or ":" in component or component == "localhost"


def _podman_archive_image_name(image_name: str) -> str:
    """Return an archive tag that matches Kubernetes' unqualified pull name."""

    repository, separator, tag = image_name.rpartition(":")
    if not separator:
        return image_name
    first_component = repository.split("/", maxsplit=1)[0]
    if _is_registry_host(first_component):
        return image_name
    if "/" in repository:
        return f"docker.io/{repository}:{tag}"
    return f"docker.io/library/{repository}:{tag}"


def _remove_stale_archive(archive_path: Path) -> None:
    """Remove the private archive directory after kind loads the image.

    The archive lives inside a per-import private directory created by
    :func:`_create_private_archive_dir`; remove that whole directory so no
    temporary files are left behind once ``podman save`` and ``kind load``
    complete.

    This runs from the ``finally`` block of the archive import, so cleanup
    failures are logged at ``WARNING`` and swallowed rather than raised: a
    cleanup error must not mask the original ``podman``/``kind`` failure that
    may already be propagating.
    """

    archive_dir = archive_path.parent

    def _log_cleanup_failure(
        function: object,
        path: str,
        excinfo: (
            BaseException
            | tuple[type[BaseException], BaseException, TracebackType | None]
        ),
    ) -> None:
        exc = excinfo[1] if isinstance(excinfo, tuple) else excinfo
        logger.warning(
            "Failed to clean up local preview image archive directory %s (%s on %s)",
            archive_dir,
            getattr(function, "__name__", function),
            path,
            exc_info=exc,
        )

    shutil.rmtree(archive_dir, onexc=_log_cleanup_failure)


def _kind_command(
    config: PreviewConfig,
    kind_args: list[str],
    *,
    use_scope: bool = False,
) -> tuple[str, list[str]]:
    """Return the command and arguments for Docker- or Podman-backed kind."""

    match (config.container_engine, use_scope):
        case ("podman", _):
            podman_args = ["KIND_EXPERIMENTAL_PROVIDER=podman", "kind", *kind_args]
            if not use_scope:
                return "env", podman_args
            return "systemd-run", [
                "--scope",
                "--user",
                "-p",
                "Delegate=yes",
                "env",
                *podman_args,
            ]
        case _:
            return "kind", kind_args
