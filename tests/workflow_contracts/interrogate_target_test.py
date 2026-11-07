"""Behavioural contracts for the Makefile's ``interrogate`` docstring gate.

The target runs the pinned tool, prints its report, and must fail both when the
tool fails and when the report holds a ``MISSED`` row, because interrogate
rounds its percentage to one decimal and a lone gap in a large repository can
still read as 100.0%. A stand-in ``uv`` prints a chosen report and exit status,
so each case runs the real recipe without the network.
"""

from __future__ import annotations

import os
import subprocess  # ruff: ignore[suspicious-subprocess-import] - tests deliberately exercise Make via subprocess.
from pathlib import Path
from shutil import which

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]


def _run_interrogate(
    tmp_path: Path, report: str, status: int
) -> subprocess.CompletedProcess[str]:
    """Run ``make interrogate`` with a ``uv`` printing ``report``, exiting ``status``.

    Returns
    -------
    subprocess.CompletedProcess[str]
        The finished ``make`` process with its captured output.
    """
    make = which("make")
    assert make is not None, "make must be available for workflow contract tests"
    fake_bin = tmp_path / "bin"
    fake_bin.mkdir()
    uv = fake_bin / "uv"
    uv.write_text(
        f"#!/bin/sh\nprintf '%s\\n' '{report}'\nexit {status}\n", encoding="utf8"
    )
    uv.chmod(0o755)
    env = os.environ.copy()
    env["PATH"] = f"{fake_bin}{os.pathsep}{env['PATH']}"
    return subprocess.run(  # ruff: ignore[subprocess-without-shell-equals-true] - the resolved make executable is trusted.
        [
            make,
            "--no-print-directory",
            "-f",
            str(REPOSITORY_ROOT / "Makefile"),
            f"PATH={env['PATH']}",
            "interrogate",
        ],
        cwd=REPOSITORY_ROOT,
        env=env,
        text=True,
        capture_output=True,
        check=False,
        timeout=30,
    )


def test_a_clean_report_passes(tmp_path: Path) -> None:
    """A clean report and a zero status pass."""
    completed = _run_interrogate(tmp_path, "| mod.py | COVERED |", 0)

    assert completed.returncode == 0, completed.stderr
    assert "| mod.py | COVERED |" in completed.stdout, "the report must be printed"


def test_a_failing_tool_fails_the_target_and_keeps_its_report(tmp_path: Path) -> None:
    """A failing tool fails the target, and its report is still printed."""
    completed = _run_interrogate(tmp_path, "RESULT: FAILED", 1)

    assert completed.returncode != 0, "a failing tool must fail the target"
    assert "RESULT: FAILED" in completed.stdout, "the failing report must be printed"


def test_a_missed_row_fails_a_report_that_passed(tmp_path: Path) -> None:
    """A MISSED row in a report from a successful run still fails the target."""
    completed = _run_interrogate(
        tmp_path, "| mod.main (L3) |  MISSED |\nRESULT: PASSED", 0
    )

    assert completed.returncode != 0, "a MISSED row must fail the target"
    assert "undocumented definitions remain" in completed.stderr, completed.stderr
    assert "mod.main (L3)" in completed.stdout, "the report must still be printed"
