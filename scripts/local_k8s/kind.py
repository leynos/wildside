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
import shutil
import tempfile
from pathlib import Path

from .config import PreviewConfig


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


def _image_archive_path(
    config: PreviewConfig, *, archive_dir: Path | None = None
) -> Path:
    """Return a Podman image archive path inside a fresh private directory.

    Creates a per-import directory via :func:`tempfile.mkdtemp`, which is owned
    by the current process with ``0700`` permissions, and returns a fixed-named
    archive path within it. Because the directory is freshly created and
    private, no other user can pre-plant a symlink at the archive path, closing
    the time-of-check/time-of-use window that a bare reserved path would leave
    open. ``archive_dir`` selects the parent under which the private directory
    is created and defaults to the system temporary directory.
    """

    base_dir = Path(tempfile.gettempdir()) if archive_dir is None else archive_dir
    private_dir = Path(
        tempfile.mkdtemp(prefix=f"{config.cluster_name}-", dir=base_dir)
    )
    return private_dir / "image.tar"


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
    :func:`_image_archive_path`; remove that whole directory so no temporary
    files are left behind once ``podman save`` and ``kind load`` complete.
    """

    shutil.rmtree(archive_path.parent, ignore_errors=True)


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
