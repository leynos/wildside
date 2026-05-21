#!/usr/bin/env -S uv run
# /// script
# requires-python = ">=3.13"
# dependencies = [
#   "cyclopts==4.10.1",
#   "plumbum==1.9.0",
# ]
# ///
"""Run the Wildside local Kubernetes preview workflow."""

from __future__ import annotations

from cyclopts import App

from local_k8s.config import PreviewConfig
from local_k8s.deployment import deploy_preview, print_logs, print_status
from local_k8s.k3d import delete_cluster
from local_k8s.validation import LocalK8sError

app = App(help="Manage a local k3d Wildside preview environment.")


def _run(operation: str, func: object) -> None:
    try:
        if callable(func):
            func()
    except LocalK8sError as exc:
        raise SystemExit(f"{operation} failed: {exc}") from exc


@app.command
def up(skip_build: bool = False) -> None:
    """Create or update the local preview environment."""

    config = PreviewConfig.from_env()
    _run("local preview up", lambda: deploy_preview(config, skip_build=skip_build))


@app.command
def down() -> None:
    """Delete the local preview cluster."""

    config = PreviewConfig.from_env()
    _run("local preview down", lambda: delete_cluster(config))


@app.command
def status() -> None:
    """Print cluster, namespace, Helm release, and pod status."""

    config = PreviewConfig.from_env()
    _run("local preview status", lambda: print_status(config))


@app.command
def logs(follow: bool = False) -> None:
    """Print logs from the Wildside backend pods."""

    config = PreviewConfig.from_env()
    _run("local preview logs", lambda: print_logs(config, follow=follow))


if __name__ == "__main__":
    app()
