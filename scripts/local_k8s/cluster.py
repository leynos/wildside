"""Provider-aware cluster lifecycle helpers for the Wildside local preview.

This module owns the command contracts for creating, deleting, inspecting, and
loading images into the Kubernetes cluster used by local preview deployments.
It keeps k3d as the default provider while adding kind support for both Docker
and rootless Podman-backed development environments.

Callers pass a fully resolved :class:`local_k8s.config.PreviewConfig` and use
the public functions as lifecycle steps:

```
ensure_cluster(config)
import_image(config)
print_cluster_status(config)
delete_cluster(config)
```

The helpers validate required tools before invoking external commands and
raise :class:`local_k8s.validation.LocalK8sError` when provider output is
malformed or the requested cluster cannot be inspected.
"""

from __future__ import annotations

import json
import logging
import tempfile
from dataclasses import dataclass
from pathlib import Path

from .commands import run
from .config import PreviewConfig
from .validation import LocalK8sError, require_tools

logger = logging.getLogger(__name__)


@dataclass(frozen=True)
class _ProviderCommandSpec:
    """Provider-specific command arguments for cluster lifecycle operations."""

    kind_args: list[str]
    k3d_args: list[str]
    kind_input_text: str | None = None
    use_scope: bool = False


def _dispatch_provider_command(
    config: PreviewConfig,
    spec: _ProviderCommandSpec,
) -> None:
    """Execute the provider-specific cluster command via kind or k3d."""
    match config.k8s_provider:
        case "kind":
            command, args = _kind_command(config, spec.kind_args, use_scope=spec.use_scope)
            logger.info(
                "local_k8s_provider_command",
                extra={
                    "provider": config.k8s_provider,
                    "cluster": config.cluster_name,
                    "command": command,
                },
            )
            run(command, args, input_text=spec.kind_input_text)
        case _:
            logger.info(
                "local_k8s_provider_command",
                extra={
                    "provider": config.k8s_provider,
                    "cluster": config.cluster_name,
                    "command": "k3d",
                },
            )
            run("k3d", spec.k3d_args)


def ensure_cluster(config: PreviewConfig) -> None:
    """Create the preview cluster when it does not already exist.

    Parameters
    ----------
    config : PreviewConfig
        Local preview settings that select the Kubernetes provider, container
        engine, cluster name, image name, and ingress port.

    Raises
    ------
    LocalK8sError
        Raised when required executables are missing or cluster creation fails.
    """

    require_tools(_cluster_tools(config))
    if _cluster_exists(config):
        print(f"{config.k8s_provider} cluster {config.cluster_name!r} already exists")
        return
    _dispatch_provider_command(
        config,
        _ProviderCommandSpec(
            kind_args=_kind_create_args(config),
            k3d_args=_k3d_create_args(config),
            kind_input_text=_kind_cluster_config(config),
            use_scope=True,
        ),
    )


def delete_cluster(config: PreviewConfig) -> None:
    """Delete the preview cluster if it exists.

    Parameters
    ----------
    config : PreviewConfig
        Local preview settings that select the Kubernetes provider and cluster
        name to delete.

    Raises
    ------
    LocalK8sError
        Raised when required executables are missing or cluster deletion fails.
    """

    require_tools((_provider_tool(config),))
    if not _cluster_exists(config):
        print(f"{config.k8s_provider} cluster {config.cluster_name!r} does not exist")
        return
    _dispatch_provider_command(
        config,
        _ProviderCommandSpec(
            kind_args=["delete", "cluster", "--name", config.cluster_name],
            k3d_args=["cluster", "delete", config.cluster_name],
        ),
    )


def import_image(config: PreviewConfig, *, archive_dir: Path | None = None) -> None:
    """Import the local backend image into the preview cluster.

    Parameters
    ----------
    config : PreviewConfig
        Local preview settings that select the Kubernetes provider, cluster
        name, and image name to import.

    Raises
    ------
    LocalK8sError
        Raised when required executables are missing or image import fails.
    """

    require_tools(_image_import_tools(config))
    logger.info(
        "local_k8s_import_image",
        extra={
            "provider": config.k8s_provider,
            "cluster": config.cluster_name,
            "image": config.image_name,
        },
    )
    match (config.k8s_provider, config.container_engine):
        case ("kind", "podman"):
            archive_path = _image_archive_path(config, archive_dir=archive_dir)
            archive_image_name = _podman_archive_image_name(config.image_name)
            _remove_stale_archive(archive_path)
            if archive_image_name != config.image_name:
                run("podman", ["tag", config.image_name, archive_image_name])
            run(
                "podman",
                [
                    "save",
                    "--output",
                    str(archive_path),
                    archive_image_name,
                ],
            )
            command, args = _kind_command(
                config,
                [
                    "load",
                    "image-archive",
                    str(archive_path),
                    "--name",
                    config.cluster_name,
                ],
            )
            run(command, args)
        case ("kind", _):
            command, args = _kind_command(
                config,
                ["load", "docker-image", config.image_name, "--name", config.cluster_name],
            )
            run(command, args)
        case _:
            run("k3d", ["image", "import", config.image_name, "--cluster", config.cluster_name])


def print_cluster_status(config: PreviewConfig) -> None:
    """Print a short description of the preview cluster.

    The status output is intentionally compact so operators can confirm the
    selected provider, cluster name, and local ingress URL before deploying or
    debugging a preview.

    Parameters
    ----------
    config : PreviewConfig
        Local preview settings that select the Kubernetes provider, cluster
        name, and ingress port to report.

    Raises
    ------
    LocalK8sError
        Raised when required executables are missing or the configured cluster
        does not exist.
    """

    require_tools((_provider_tool(config),))
    logger.info(
        "local_k8s_cluster_status",
        extra={
            "provider": config.k8s_provider,
            "cluster": config.cluster_name,
        },
    )
    if not _cluster_exists(config):
        error_message = f"{config.k8s_provider} cluster {config.cluster_name!r} does not exist"
        raise LocalK8sError(error_message)
    print(f"cluster: {config.cluster_name}")
    print(f"provider: {config.k8s_provider}")
    if config.k8s_provider == "kind":
        print(f"port-forward address: http://127.0.0.1:{config.ingress_port}")
    else:
        print(f"ingress: http://127.0.0.1:{config.ingress_port}")


def _cluster_tools(config: PreviewConfig) -> tuple[str, ...]:
    """Return command-line tools needed to manage the configured cluster."""

    match (config.k8s_provider, config.container_engine):
        case ("kind", "podman"):
            return ("kind", "podman", "kubectl", "helm", "systemd-run")
        case ("kind", _):
            return ("kind", "kubectl", "helm")
        case _:
            return ("k3d", "kubectl", "helm")


def _provider_tool(config: PreviewConfig) -> str:
    """Return the executable that owns the configured cluster provider."""

    match config.k8s_provider:
        case "kind":
            return "kind"
        case _:
            return "k3d"


def _image_import_tools(config: PreviewConfig) -> tuple[str, ...]:
    """Return tools needed to import the local image into the cluster."""

    match (config.k8s_provider, config.container_engine):
        case ("kind", "podman"):
            return ("kind", "podman")
        case _:
            return (_provider_tool(config),)


def _cluster_exists(config: PreviewConfig) -> bool:
    """Return whether the configured provider reports the preview cluster."""

    match config.k8s_provider:
        case "kind":
            command, args = _kind_command(config, ["get", "clusters"])
            result = run(command, args)
            return config.cluster_name in result.stdout.splitlines()
        case _:
            return _k3d_cluster_exists(config.cluster_name)


def _k3d_cluster_exists(cluster_name: str) -> bool:
    """Return whether k3d reports a cluster with the given name."""

    result = run("k3d", ["cluster", "list", "--output", "json"])
    try:
        clusters = json.loads(result.stdout or "[]")
    except json.JSONDecodeError as exc:
        error_message = "unexpected k3d cluster list JSON payload"
        raise LocalK8sError(error_message) from exc
    match clusters:
        case list():
            return any(
                _cluster_name_from_k3d_payload(cluster) == cluster_name
                for cluster in clusters
            )
        case _:
            error_message = "unexpected k3d cluster list JSON shape"
            raise LocalK8sError(error_message)


def _cluster_name_from_k3d_payload(cluster: object) -> str | None:
    """Return a k3d cluster name from a decoded JSON cluster payload."""

    match cluster:
        case {"name": str(name)}:
            return name
        case _:
            return None


def _k3d_create_args(config: PreviewConfig) -> list[str]:
    """Return the k3d cluster creation arguments."""

    return [
        "cluster",
        "create",
        config.cluster_name,
        "--servers",
        "1",
        "--agents",
        "1",
        "--port",
        f"127.0.0.1:{config.ingress_port}:80@loadbalancer",
        "--wait",
    ]


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


def _image_archive_path(config: PreviewConfig, *, archive_dir: Path | None = None) -> Path:
    """Return the temporary Podman image archive path for kind loading."""

    base_dir = Path(tempfile.gettempdir()) if archive_dir is None else archive_dir
    return base_dir / f"{config.cluster_name}-image.tar"


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
    """Remove a stale image archive before writing a fresh Podman export."""

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
            return "systemd-run", ["--scope", "--user", "-p", "Delegate=yes", "env", *podman_args]
        case _:
            return "kind", kind_args
