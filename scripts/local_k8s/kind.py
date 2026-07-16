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
import os
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


def _reserve_unique_temp_path(*, prefix: str, suffix: str, base_dir: Path) -> Path:
    """Reserve a unique filename by creating and immediately removing a temp file.

    Side effect: creates a temporary file via :func:`tempfile.mkstemp` to claim a
    collision-free name, closes the descriptor, then unlinks it so the caller can
    create the real artefact at that path. Returns the reserved path.
    """

    file_descriptor, reserved = tempfile.mkstemp(
        prefix=prefix,
        suffix=suffix,
        dir=base_dir,
    )
    os.close(file_descriptor)
    path = Path(reserved)
    path.unlink(missing_ok=True)
    return path


def _image_archive_path(
    config: PreviewConfig, *, archive_dir: Path | None = None
) -> Path:
    """Return a unique temporary Podman image archive path for kind loading."""

    base_dir = Path(tempfile.gettempdir()) if archive_dir is None else archive_dir
    return _reserve_unique_temp_path(
        prefix=f"{config.cluster_name}-",
        suffix="-image.tar",
        base_dir=base_dir,
    )


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
    """Remove the temporary Podman image archive after kind loads it."""

    archive_path.unlink(missing_ok=True)


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
