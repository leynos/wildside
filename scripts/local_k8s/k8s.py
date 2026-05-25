"""Kubernetes helper operations for the Wildside local preview."""

from __future__ import annotations

from .commands import run
from .config import PreviewConfig
from .validation import require_tools


def ensure_namespace(config: PreviewConfig) -> None:
    """Create the preview namespace when it does not already exist."""

    require_tools(("kubectl",))
    result = run("kubectl", ["get", "namespace", config.namespace, "--ignore-not-found"])
    if result.stdout.strip():
        return
    run("kubectl", ["create", "namespace", config.namespace])


def _helm_fullname(config: PreviewConfig) -> str:
    """Return the chart fullname used for Kubernetes object names."""

    chart_name = config.chart_path.name
    if config.release_name == chart_name:
        return chart_name
    return f"{config.release_name}-{chart_name}"[:63].rstrip("-")


def print_kubernetes_status(config: PreviewConfig) -> None:
    """Print namespace, service, and pod status for the preview release."""

    require_tools(("kubectl",))
    print(f"namespace: {config.namespace}")
    pods = run(
        "kubectl",
        [
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
            "-n",
            config.namespace,
            "get",
            "service",
            _helm_fullname(config),
            "--ignore-not-found",
        ],
    )
    print(services.stdout.strip() or "service: none")
