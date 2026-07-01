"""Build, deploy, inspect, and log the Wildside local preview."""

from __future__ import annotations

import base64
import logging
import secrets
from collections.abc import Callable

from .commands import run
from .config import PreviewConfig
from .cluster import ensure_cluster, import_image, print_cluster_status
from .k8s import ensure_namespace, helm_fullname, print_kubernetes_status
from .validation import LocalK8sError, require_tools

SESSION_SECRET_KEY_NAME = "session_key"  # noqa: S105 - Secret data key name, not secret material.
SESSION_SECRET_NAME = "wildside-session-key"  # noqa: S105 - Secret resource name, not secret material.

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


def _fetch_existing_session_key(config: PreviewConfig) -> str:
    """Return the existing local session key payload, when Kubernetes has one."""
    existing_key = run(
        "kubectl",
        [
            "--context",
            config.kube_context,
            "-n",
            config.namespace,
            "get",
            "secret",
            SESSION_SECRET_NAME,
            "--ignore-not-found",
            f"-o=jsonpath={{.data.{SESSION_SECRET_KEY_NAME}}}",
        ],
    )
    return existing_key.stdout.strip()


def _render_session_secret_manifest(config: PreviewConfig, encoded_key: str) -> str:
    """Render the Kubernetes Secret manifest for the generated session key."""
    return f"""\
apiVersion: v1
kind: Secret
metadata:
  name: {SESSION_SECRET_NAME}
  namespace: {config.namespace}
type: Opaque
data:
  {SESSION_SECRET_KEY_NAME}: {encoded_key}
"""


def _log_session_secret_event(config: PreviewConfig, event: str) -> None:
    """Log a session Secret lifecycle event with the standard preview fields."""
    logger.info(
        event,
        extra={
            "cluster": config.cluster_name,
            "namespace": config.namespace,
            "secret": SESSION_SECRET_NAME,
        },
    )


def _apply_session_secret_manifest(config: PreviewConfig, manifest: str) -> None:
    """Create the session Secret, treating concurrent creation as reuse."""
    _log_session_secret_event(config, "local_k8s_session_secret_apply")
    try:
        run(
            "kubectl",
            [
                "--context",
                config.kube_context,
                "create",
                "-f",
                "-",
            ],
            input_text=manifest,
        )
    except LocalK8sError as exc:
        if "already exists" in str(exc):
            _log_session_secret_event(config, "local_k8s_session_secret_reuse")
            return
        raise


def ensure_session_secret(
    config: PreviewConfig,
    *,
    key_generator: Callable[[int], bytes] = secrets.token_bytes,
) -> None:
    """Create the local preview session signing key Secret when absent.

    Parameters
    ----------
    config : PreviewConfig
        Local preview settings that select the kube context, namespace, and
        Secret location used by the deployment.
    key_generator : Callable[[int], bytes], optional
        Injectable source of key bytes. Tests use this seam to make the
        rendered Secret deterministic; production uses ``secrets.token_bytes``.

    Returns
    -------
    None
        The function applies a Kubernetes Secret only when the expected key is
        missing. Existing key material is reused to avoid rotating local
        preview sessions on every deployment.
    """

    if _fetch_existing_session_key(config):
        _log_session_secret_event(config, "local_k8s_session_secret_reuse")
        return

    key = key_generator(96)
    encoded_key = base64.b64encode(key).decode("ascii")
    manifest = _render_session_secret_manifest(config, encoded_key)
    _apply_session_secret_manifest(config, manifest)


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
    """Print the kind port-forward command.

    Parameters
    ----------
    config
        Preview configuration describing the selected provider, kube context,
        namespace, release name, and ingress port.

    Returns
    -------
    None
        Prints the `kubectl port-forward` command for `kind` previews. Returns
        silently for other providers, including `k3d`.
    """

    if config.k8s_provider != "kind":
        return
    print("port-forward:")
    print(
        f"kubectl --context {config.kube_context} --namespace {config.namespace} "
        f"port-forward svc/{helm_fullname(config)} {config.ingress_port}:80"
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
