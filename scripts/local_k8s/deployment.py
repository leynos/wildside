"""Build, deploy, inspect, and log the Wildside local preview."""

from __future__ import annotations

from .commands import run
from .config import PreviewConfig
from .k3d import ensure_cluster, import_image, print_cluster_status
from .k8s import ensure_namespace, print_kubernetes_status
from .validation import LocalK8sError, require_tools


def deploy_preview(config: PreviewConfig, *, skip_build: bool) -> None:
    """Build the image and install or upgrade the Wildside Helm release."""

    require_tools(_deploy_preview_tools(skip_build=skip_build))
    ensure_cluster(config)
    ensure_namespace(config)
    if not skip_build:
        build_image(config)
    import_image(config)
    helm_upgrade(config)
    print_status(config)


def _deploy_preview_tools(*, skip_build: bool) -> tuple[str, ...]:
    """Return the required command-line tools for the requested deploy mode."""

    if skip_build:
        return ("helm", "k3d", "kubectl")
    return ("docker", "helm", "k3d", "kubectl")


def build_image(config: PreviewConfig) -> None:
    """Build the Wildside backend image for local k3d import."""

    run(
        "docker",
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
    """Install or upgrade the Wildside Helm release."""

    image_repository, image_tag = image_repository_and_tag(config.image_name)
    run(
        "helm",
        [
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

    require_tools(("helm", "k3d", "kubectl"))
    print_cluster_status(config)
    release = run("helm", ["-n", config.namespace, "status", config.release_name])
    print(release.stdout.strip())
    print_kubernetes_status(config)


def print_logs(config: PreviewConfig, *, follow: bool) -> None:
    """Print backend pod logs from the preview namespace."""

    require_tools(("kubectl",))
    args = [
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
