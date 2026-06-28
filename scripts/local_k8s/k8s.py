"""Kubernetes helper operations for the Wildside local preview."""

from __future__ import annotations

from .commands import run
from .config import PreviewConfig
from .validation import require_tools


def ensure_namespace(config: PreviewConfig) -> None:
    """Create the preview namespace when absent.

    Parameters
    ----------
    config : PreviewConfig
        Preview configuration carrying the target namespace.

    Returns
    -------
    None
        Mutates cluster state only when the namespace is missing.

    Raises
    ------
    LocalK8sError
        Raised when ``kubectl`` is unavailable or command execution fails.
    """

    require_tools(("kubectl",))
    result = run(
        "kubectl",
        [
            "--context",
            config.kube_context,
            "get",
            "namespace",
            config.namespace,
            "--ignore-not-found",
        ],
    )
    if result.stdout.strip():
        return
    run("kubectl", ["--context", config.kube_context, "create", "namespace", config.namespace])


def helm_fullname(config: PreviewConfig) -> str:
    """Return the chart fullname used for Kubernetes object names.

    Parameters
    ----------
    config : PreviewConfig
        Preview configuration containing the Helm release and chart names.

    Returns
    -------
    str
        Helm-compatible fullname, truncated to the Kubernetes DNS label limit.
    """

    chart_name = config.chart_path.name
    if config.release_name == chart_name:
        return chart_name
    return f"{config.release_name}-{chart_name}"[:63].rstrip("-")


def print_kubernetes_status(config: PreviewConfig) -> None:
    """Print Kubernetes status for the preview release.

    Parameters
    ----------
    config : PreviewConfig
        Preview configuration containing the namespace and Helm release name.

    Returns
    -------
    None
        Status is printed to standard output.

    Raises
    ------
    LocalK8sError
        Raised when ``kubectl`` is unavailable or command execution fails.
    """

    require_tools(("kubectl",))
    print(f"namespace: {config.namespace}")
    pods = run(
        "kubectl",
        [
            "--context",
            config.kube_context,
            "-n",
            config.namespace,
            "get",
            "pods",
            "-l",
            f"app.kubernetes.io/instance={config.release_name}",
            "-o",
            "wide",
        ],
    )
    print(pods.stdout.strip() or "pods: none")
    services = run(
        "kubectl",
        [
            "--context",
            config.kube_context,
            "-n",
            config.namespace,
            "get",
            "service",
            helm_fullname(config),
            "--ignore-not-found",
        ],
    )
    print(services.stdout.strip() or "service: none")
