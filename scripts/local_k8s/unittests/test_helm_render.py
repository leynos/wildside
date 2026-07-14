"""Helm render smoke checks for the local preview chart values.

These render the wildside chart with ``values.local.yaml`` and assert the
session-secret mount path and session-key environment survive templating, so a
values regression that breaks the local preview is caught before deploy.
"""

from __future__ import annotations

import shutil
import subprocess
from pathlib import Path
from typing import Any

import pytest
import yaml

REPO_ROOT = Path(__file__).resolve().parents[3]
CHART_DIR = REPO_ROOT / "deploy" / "charts" / "wildside"
LOCAL_VALUES = CHART_DIR / "values.local.yaml"
SESSION_MOUNT_PATH = "/var/run/secrets/wildside-session"


def _manifests(rendered: str) -> list[dict[str, Any]]:
    """Return the rendered multi-document YAML as a list of manifest mappings."""
    return [doc for doc in yaml.safe_load_all(rendered) if isinstance(doc, dict)]


def _first_of_kind(manifests: list[dict[str, Any]], kind: str) -> dict[str, Any]:
    """Return the first rendered manifest with the given ``kind``."""
    return next(manifest for manifest in manifests if manifest.get("kind") == kind)


@pytest.fixture(scope="module")
def local_preview_render() -> str:
    """Render the wildside chart with the local preview values."""
    helm = shutil.which("helm")
    if helm is None:
        pytest.skip("helm is required to render the local preview chart")
    completed = subprocess.run(  # noqa: S603 - argv is fixed by the test.
        [
            helm,
            "template",
            "wildside",
            str(CHART_DIR),
            "--values",
            str(LOCAL_VALUES),
            "--kube-version",
            "1.31.0",
        ],
        capture_output=True,
        text=True,
        check=True,
    )
    return completed.stdout


def test_local_render_mounts_session_secret_path(local_preview_render: str) -> None:
    """Verify the rendered manifests mount the local session secret path."""
    assert f"mountPath: {SESSION_MOUNT_PATH}" in local_preview_render, (
        "local preview render must mount the session secret at the configured path"
    )


def test_local_render_wires_session_key_env(local_preview_render: str) -> None:
    """Verify the backend resolves SESSION_KEY_FILE to the mounted session key."""
    manifests = _manifests(local_preview_render)
    deployment = _first_of_kind(manifests, "Deployment")
    config_map = _first_of_kind(manifests, "ConfigMap")

    containers = deployment["spec"]["template"]["spec"]["containers"]
    env_by_name = {
        entry["name"]: entry
        for container in containers
        for entry in container.get("env", [])
    }
    session_env = env_by_name["SESSION_KEY_FILE"]
    config_map_key = session_env["valueFrom"]["configMapKeyRef"]["key"]

    assert config_map["data"][config_map_key] == f"{SESSION_MOUNT_PATH}/session_key", (
        "SESSION_KEY_FILE must resolve to the mounted session key path"
    )
