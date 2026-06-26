"""Smoke tests for the local Kubernetes preview CLI boundary."""

from __future__ import annotations

import subprocess
from pathlib import Path
from shutil import which


def test_local_k8s_cli_help_smoke() -> None:
    """Verify the script entry point loads and exposes the preview CLI."""
    uv = which("uv")
    assert uv is not None
    script_path = Path(__file__).resolve().parents[2] / "local_k8s.py"

    completed = subprocess.run(  # noqa: S603 - argv is fixed by the test.
        [uv, "run", str(script_path), "--help"],
        text=True,
        capture_output=True,
        check=True,
        timeout=60,
    )

    assert "Manage a local Kubernetes Wildside preview environment." in completed.stdout
