"""Shared harness for the embedded PostgreSQL cache warm-up script tests.

The warm-up logic is a Bash script, so every test sources it and runs a snippet
in a subshell. Both test modules need that harness, so it lives here rather
than being duplicated or imported across test modules.
"""

from __future__ import annotations

import os
import subprocess  # noqa: S404 -- test harness invokes a fixed, trusted script
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = PROJECT_ROOT / "scripts" / "warm-pg-embedded-cache.sh"


def result_diagnostics(result: subprocess.CompletedProcess[str]) -> str:
    """Format subprocess output for actionable assertion failures."""
    return (
        f"returncode={result.returncode}; "
        f"stdout={result.stdout!r}; stderr={result.stderr!r}"
    )


def run_bash(
    snippet: str,
    *,
    env: dict[str, str] | None = None,
    timeout: float = 5.0,
) -> subprocess.CompletedProcess[str]:
    """Source the warm-up script and run a Bash snippet."""
    merged_env = os.environ.copy()
    merged_env.pop("PG_EMBEDDED_VERSION", None)
    merged_env.pop("POSTGRESQL_VERSION", None)
    merged_env.pop("PG_BINARY_CACHE_DIR", None)
    merged_env.pop("POSTGRESQL_RELEASES_URL", None)
    if env is not None:
        merged_env.update(env)
    return subprocess.run(  # noqa: S603 -- args are test-controlled, not external input
        # S607 below: bash is required to source SCRIPT_PATH before running the
        # snippet, and is resolved from PATH so the platform's shell wins.
        ["bash", "-c", f"source {SCRIPT_PATH} && {snippet}"],  # noqa: S607
        cwd=PROJECT_ROOT,
        env=merged_env,
        text=True,
        capture_output=True,
        timeout=timeout,
        check=False,
    )
