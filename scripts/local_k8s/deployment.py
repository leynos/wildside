"""Build, deploy, inspect, and log the Wildside local preview."""

from __future__ import annotations

import base64
import logging
import secrets
from collections.abc import Callable

from .commands import run
from .config import PreviewConfig
from .cluster import ensure_cluster, import_image, print_cluster_status
from .k8s import _helm_fullname, ensure_namespace, print_kubernetes_status
from .validation import LocalK8sError, require_tools

SESSION_SECRET_KEY_NAME = "session_key"
SESSION_SECRET_NAME = "wildside-session-key"

logger = logging.getLogger(__name__)


def deploy_preview(config: PreviewConfig, *, skip_build: bool) -> None:
    """Build the image and install or upgrade the Wildside Helm release."""

    require_tools(_deploy_preview_tools(config, skip_build=skip_build))
    logger.info(
        "local_k8s_deploy_preview",
        extra={
            "provider": config.k8s_provider,
            "cluster": config.cluster_name,
            "release": config.release_name,
            "image": config.image_name,
            "skip_build": skip_build,
        },
    )
    ensure_cluster(config)
    ensure_namespace(config)
    ensure_session_secret(config)
    if not skip_build:
        build_image(config)
    import_image(config)
    helm_upgrade(config)
    print_status(config)


def _deploy_preview_tools(config: PreviewConfig, *, skip_build: bool) -> tuple[str, ...]:
    """Return the required command-line tools for the requested deploy mode."""

    cluster_tool = "kind" if config.k8s_provider == "kind" else "k3d"
    if skip_build:
        return ("helm", cluster_tool, "kubectl")
    return (config.container_engine, "helm", cluster_tool, "kubectl")


def build_image(config: PreviewConfig) -> None:
    """Build the Wildside backend image for local preview import."""

    logger.info(
        "local_k8s_build_image",
        extra={
            "engine": config.container_engine,
            "image": config.image_name,
        },
    )
    run(
        config.container_engine,
        [
            "build",
            "-f",
            str(config.dockerfile_path),
            "-t",
            config.image_name,
            str(config.repository_root),
        ],
    )


def ensure_session_secret(
    config: PreviewConfig,
    *,
    key_generator: Callable[[int], bytes] = secrets.token_bytes,
) -> None:
    """Create or refresh the local preview session signing key Secret."""

    logger.info(
        "local_k8s_session_secret_apply",
        extra={
            "cluster": config.cluster_name,
            "namespace": config.namespace,
            "secret": SESSION_SECRET_NAME,
        },
    )
    key = key_generator(96)
    encoded_key = base64.b64encode(key).decode("ascii")
    manifest = f"""\
apiVersion: v1
kind: Secret
metadata:
  name: {SESSION_SECRET_NAME}
  namespace: {config.namespace}
type: Opaque
data:
  {SESSION_SECRET_KEY_NAME}: {encoded_key}
"""
    run(
        "kubectl",
        [
            "--context",
            config.kube_context,
            "apply",
            "-f",
            "-",
        ],
        input_text=manifest,
    )


def helm_upgrade(config: PreviewConfig) -> None:
    """Install or upgrade the Wildside Helm release."""

    image_repository, image_tag = image_repository_and_tag(config.image_name)
    logger.info(
        "local_k8s_helm_upgrade",
        extra={
            "cluster": config.cluster_name,
            "release": config.release_name,
            "image": config.image_name,
        },
    )
    run(
        "helm",
        [
            "--kube-context",
            config.kube_context,
            "upgrade",
            "--install",
            config.release_name,
            str(config.chart_path),
            "--namespace",
            config.namespace,
            "--values",
            str(config.local_values_path),
            "--set",
            f"image.repository={image_repository}",
            "--set",
            f"image.tag={image_tag}",
            "--wait",
            "--timeout",
            "5m",
        ],
    )


def _image_ref_lacks_tag(repository: str, separator: str, tag: str) -> bool:
    """Return True when the parsed parts do not form a valid image:tag reference."""
    return not separator or "/" in tag or not repository or not tag


def image_repository_and_tag(image_name: str) -> tuple[str, str]:
    """Split a Docker image reference into Helm repository and tag values."""

    repository, separator, tag = image_name.rpartition(":")
    if _image_ref_lacks_tag(repository, separator, tag):
        raise LocalK8sError(
            "WILDSIDE_IMAGE must include a tag, for example wildside-backend:local"
        )
    return repository, tag


def print_status(config: PreviewConfig) -> None:
    """Print cluster and workload status."""

    require_tools(_deploy_preview_tools(config, skip_build=True))
    logger.info(
        "local_k8s_print_status",
        extra={
            "provider": config.k8s_provider,
            "cluster": config.cluster_name,
            "release": config.release_name,
        },
    )
    print_cluster_status(config)
    release = run(
        "helm",
        [
            "--kube-context",
            config.kube_context,
            "-n",
            config.namespace,
            "status",
            config.release_name,
        ],
    )
    print(release.stdout.strip())
    print_kubernetes_status(config)
    print_kind_port_forward_command(config)


def print_kind_port_forward_command(config: PreviewConfig) -> None:
    """Print the port-forward command needed for kind previews."""

    if config.k8s_provider != "kind":
        return
    print("port-forward:")
    print(
        f"kubectl --context {config.kube_context} --namespace {config.namespace} "
        f"port-forward svc/{_helm_fullname(config)} {config.ingress_port}:80"
    )


def print_logs(config: PreviewConfig, *, follow: bool) -> None:
    """Print backend pod logs from the preview namespace."""

    require_tools(("kubectl",))
    args = [
        "--context",
        config.kube_context,
        "-n",
        config.namespace,
        "logs",
        "-l",
        f"app.kubernetes.io/instance={config.release_name}",
        "-c",
        "app",
        "--tail",
        "200",
    ]
    if follow:
        args.append("--follow")
    print(run("kubectl", args).stdout, end="")
