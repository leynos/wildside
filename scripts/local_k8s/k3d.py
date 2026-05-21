"""k3d cluster lifecycle helpers for the Wildside local preview."""

from __future__ import annotations

import json

from .commands import run
from .config import PreviewConfig
from .validation import LocalK8sError, require_tools


def ensure_cluster(config: PreviewConfig) -> None:
    """Create the preview cluster when it does not already exist."""

    require_tools(("k3d", "kubectl", "helm"))
    if _cluster_exists(config.cluster_name):
        print(f"k3d cluster {config.cluster_name!r} already exists")
        return
    args = [
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
    run("k3d", args)


def delete_cluster(config: PreviewConfig) -> None:
    """Delete the preview cluster if it exists."""

    require_tools(("k3d",))
    if not _cluster_exists(config.cluster_name):
        print(f"k3d cluster {config.cluster_name!r} does not exist")
        return
    run("k3d", ["cluster", "delete", config.cluster_name])


def import_image(config: PreviewConfig) -> None:
    """Import the local backend image into the preview cluster."""

    require_tools(("k3d",))
    run("k3d", ["image", "import", config.image_name, "--cluster", config.cluster_name])


def print_cluster_status(config: PreviewConfig) -> None:
    """Print a short description of the preview cluster."""

    require_tools(("k3d",))
    if not _cluster_exists(config.cluster_name):
        raise LocalK8sError(f"k3d cluster {config.cluster_name!r} does not exist")
    print(f"cluster: {config.cluster_name}")
    print(f"ingress: http://127.0.0.1:{config.ingress_port}")


def _cluster_exists(cluster_name: str) -> bool:
    result = run("k3d", ["cluster", "list", "--output", "json"])
    clusters = json.loads(result.stdout or "[]")
    if not isinstance(clusters, list):
        raise LocalK8sError("unexpected k3d cluster list JSON shape")
    return any(isinstance(cluster, dict) and cluster.get("name") == cluster_name for cluster in clusters)
