"""Helm render smoke checks for the local preview chart values.

These render the wildside chart with ``values.local.yaml`` and assert the
session-secret mount path and session-key environment survive templating, so a
values regression that breaks the local preview is caught before deploy.
"""

from __future__ import annotations

import shutil
import subprocess
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[3]
CHART_DIR = REPO_ROOT / "deploy" / "charts" / "wildside"
LOCAL_VALUES = CHART_DIR / "values.local.yaml"
SESSION_MOUNT_PATH = "/var/run/secrets/wildside-session"


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
    """Verify the rendered manifests expose the session key file environment."""
    assert "SESSION_KEY_FILE" in local_preview_render, (
        "local preview render must expose SESSION_KEY_FILE to the backend"
    )
    assert f"{SESSION_MOUNT_PATH}/session_key" in local_preview_render, (
        "SESSION_KEY_FILE must point at the mounted session key"
    )
