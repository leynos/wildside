"""Build, deploy, inspect, and log the Wildside local preview."""

from __future__ import annotations

import logging
import re
import shlex
import typing as typ

from .cluster import ensure_cluster, import_image, print_cluster_status
from .commands import run, run_streaming
from .k8s import ensure_namespace, helm_fullname, print_kubernetes_status
from .session_secret import ensure_session_secret
from .validation import LocalK8sError, require_tools

if typ.TYPE_CHECKING:
    from .config import PreviewConfig

# ``ensure_session_secret`` is re-exported so callers (and tests that patch
# ``local_k8s.deployment.ensure_session_secret``) keep resolving it here.
__all__ = ["ensure_session_secret"]

# Helm's ``--set`` splits values on commas and treats ``=`` as a key/value
# separator, so an image reference containing those (or backslash/brace)
# characters could smuggle additional chart values into the release. Constrain
# the repository and tag to an OCI-safe grammar that excludes every Helm
# ``--set`` metacharacter before forwarding them.
_IMAGE_REPOSITORY_PATTERN = re.compile(r"[A-Za-z0-9][A-Za-z0-9._:/@-]{0,254}")
_IMAGE_TAG_PATTERN = re.compile(r"[A-Za-z0-9_][A-Za-z0-9._-]{0,127}")

logger = logging.getLogger(__name__)


def deploy_preview(config: PreviewConfig, *, skip_build: bool) -> None:
    """Build the image and install or upgrade the Wildside Helm release.

    Parameters
    ----------
    config : PreviewConfig
        Local preview settings that select the runtime tools, cluster,
        namespace, image, and Helm release.
    skip_build : bool
        When ``True``, reuse the currently tagged local image instead of
        rebuilding it before import.

    Notes
    -----
    Deployment ensures the cluster, namespace, and session-signing Secret
    before building or importing the image. This order keeps
    ``ensure_session_secret`` idempotent and avoids rotating local preview
    sessions during normal Helm upgrades.
    """
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


def _deploy_preview_tools(
    config: PreviewConfig, *, skip_build: bool
) -> tuple[str, ...]:
    """Return the required command-line tools for the requested deploy mode."""
    match config.k8s_provider:
        case "kind":
            cluster_tool = "kind"
        case "k3d":
            cluster_tool = "k3d"
        case unexpected:
            error_message = f"Unsupported Kubernetes provider: {unexpected!r}"
            raise LocalK8sError(error_message)
    if skip_build:
        return ("helm", cluster_tool, "kubectl")
    return (config.container_engine, "helm", cluster_tool, "kubectl")


def build_image(config: PreviewConfig) -> None:
    """Build the Wildside backend image for local preview import.

    The image is built with the configured container engine
    (``config.container_engine``, either ``docker`` or ``podman``) from
    ``config.dockerfile_path``, using ``config.repository_root`` as the build
    context and tagging the result ``config.image_name``.

    Parameters
    ----------
    config : PreviewConfig
        Local preview settings that select the container engine, image name,
        Dockerfile path, and repository root used for the build.
    """
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


def helm_upgrade(config: PreviewConfig) -> None:
    """Install or upgrade the Wildside Helm release.

    Runs ``helm upgrade --install`` against the provider-derived Kubernetes
    context (``config.kube_context``) in ``config.namespace``, deploying the
    chart at ``config.chart_path`` with ``config.local_values_path`` and the
    repository and tag split from ``config.image_name``.

    Parameters
    ----------
    config : PreviewConfig
        Local preview settings that select the kube context, namespace, Helm
        release name, chart path, local values file, and image reference.
    """
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
            # Use --set-string so the validated repository and tag are always
            # passed as literal strings, never coerced (e.g. a numeric tag).
            "--set-string",
            f"image.repository={image_repository}",
            "--set-string",
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
    """Split a Docker image reference into Helm repository and tag values.

    The repository and tag are validated against an OCI-safe grammar before
    they are returned so that neither can carry characters that Helm's
    ``--set``/``--set-string`` flags treat specially (commas, ``=``, braces,
    or backslashes), preventing chart-value injection via ``WILDSIDE_IMAGE``.

    Parameters
    ----------
    image_name : str
        Fully tagged image reference (``repository:tag``) to split, typically
        sourced from ``WILDSIDE_IMAGE``.

    Returns
    -------
    tuple[str, str]
        The validated ``(repository, tag)`` pair extracted from ``image_name``.

    Raises
    ------
    LocalK8sError
        Raised when ``image_name`` omits a tag, or when the repository or tag
        fails OCI-safe grammar validation.
    """
    repository, separator, tag = image_name.rpartition(":")
    if _image_ref_lacks_tag(repository, separator, tag):
        msg = "WILDSIDE_IMAGE must include a tag, for example wildside-backend:local"
        raise LocalK8sError(msg)
    if _IMAGE_REPOSITORY_PATTERN.fullmatch(repository) is None:
        msg = (
            "WILDSIDE_IMAGE repository must be a valid OCI reference without "
            "whitespace or Helm --set metacharacters (',', '=', '{', '}', '\\')"
        )
        raise LocalK8sError(msg)
    if _IMAGE_TAG_PATTERN.fullmatch(tag) is None:
        msg = (
            "WILDSIDE_IMAGE tag must be a valid OCI tag without whitespace or "
            "Helm --set metacharacters (',', '=', '{', '}', '\\')"
        )
        raise LocalK8sError(msg)
    return repository, tag


def print_status(config: PreviewConfig) -> None:
    """Print the local preview cluster and workload status.

    Parameters
    ----------
    config : PreviewConfig
        Local preview settings selecting the provider, kube context, namespace,
        and Helm release inspected for status.

    Returns
    -------
    None
        Prints the provider cluster status, the ``helm status`` output for the
        release, and the Kubernetes workload status to standard output.

    Raises
    ------
    LocalK8sError
        Raised when a required tool is missing or a status command fails.
    """
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
    config : PreviewConfig
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
    # Build the hint from separately quoted argv segments so no unescaped shell
    # string is ever emitted, even though the underlying names are validated as
    # DNS-1123 labels upstream in PreviewConfig.
    command = shlex.join([
        "kubectl",
        "--context",
        config.kube_context,
        "--namespace",
        config.namespace,
        "port-forward",
        f"svc/{helm_fullname(config)}",
        f"{config.ingress_port}:80",
    ])
    print("port-forward:")
    print(command)


def print_logs(config: PreviewConfig, *, follow: bool) -> None:
    """Print backend pod logs from the preview namespace.

    Parameters
    ----------
    config : PreviewConfig
        Local preview settings selecting the kube context, namespace, and Helm
        release whose backend pod logs are read.
    follow : bool
        When true, stream logs continuously (``kubectl logs --follow``);
        otherwise print the most recent lines once.

    Returns
    -------
    None
        Prints (or streams) the backend pod logs to standard output.

    Raises
    ------
    LocalK8sError
        Raised when ``kubectl`` is missing or the logs command fails.
    """
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
        run_streaming("kubectl", args)
        return
    print(run("kubectl", args).stdout, end="")
