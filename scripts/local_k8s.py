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

import typing as typ

from cyclopts import App
from local_k8s.cluster import delete_cluster
from local_k8s.config import PreviewConfig
from local_k8s.deployment import deploy_preview, print_logs, print_status
from local_k8s.validation import LocalK8sError

if typ.TYPE_CHECKING:
    import collections.abc as cabc

app = App(help="Manage a local Kubernetes Wildside preview environment.")


def _run(operation: str, func: cabc.Callable[[], None]) -> None:
    """Run *func*, translating any `LocalK8sError` into a `SystemExit`.

    Parameters
    ----------
    operation : str
        Human-readable name of the operation, used in the error message.
    func : cabc.Callable[[], None]
        Zero-argument callable to invoke.

    Raises
    ------
    SystemExit
        If *func* raises `LocalK8sError`.
    """
    try:
        func()
    except LocalK8sError as exc:
        message = f"{operation} failed: {exc}"
        raise SystemExit(message) from exc


@app.command
def up(*, skip_build: bool = False) -> None:
    """Create or update the local preview environment."""
    _run(
        "local preview up",
        lambda: deploy_preview(PreviewConfig.from_env(), skip_build=skip_build),
    )


@app.command
def down() -> None:
    """Delete the local preview cluster."""
    _run("local preview down", lambda: delete_cluster(PreviewConfig.from_env()))


@app.command
def status() -> None:
    """Print cluster, namespace, Helm release, and pod status."""
    _run("local preview status", lambda: print_status(PreviewConfig.from_env()))


@app.command
def logs(*, follow: bool = False) -> None:
    """Print logs from the Wildside backend pods."""
    _run(
        "local preview logs",
        lambda: print_logs(PreviewConfig.from_env(), follow=follow),
    )


if __name__ == "__main__":
    app()
