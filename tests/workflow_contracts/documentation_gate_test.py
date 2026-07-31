"""Contract tests for the zero-tolerance TypeDoc documentation gate."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[2]
PACKAGE_MANIFEST = PROJECT_ROOT / "package.json"
MAKEFILE = PROJECT_ROOT / "Makefile"
FIXTURE_CONFIG = Path(__file__).parent / "fixtures" / "typedoc.json"

EXPECTED_TYPEDOC_COMMANDS = [
    "typedoc --options frontend-pwa/typedoc.json",
    "typedoc --options packages/types/typedoc.json",
    "typedoc --options packages/tokens/typedoc.json",
]


def test_docs_check_runs_all_three_typedoc_configs() -> None:
    """The package script must validate every maintained TypeDoc surface."""
    manifest = json.loads(PACKAGE_MANIFEST.read_text(encoding="utf-8"))
    command = manifest["scripts"]["docs:check"]
    assert command.split(" && ") == EXPECTED_TYPEDOC_COMMANDS


def test_make_targets_keep_docs_check_in_the_repository_gate() -> None:
    """The Makefile must expose TypeDoc and retain it in the aggregate gate."""
    makefile = MAKEFILE.read_text(encoding="utf-8")
    assert "all: check-fmt lint docs-check test spelling" in makefile

    completed = subprocess.run(  # noqa: S603 - fixed local build command.
        ["make", "--dry-run", "docs-check"],
        cwd=PROJECT_ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    assert "pnpm run docs:check" in completed.stdout.splitlines()


def test_typedoc_rejects_an_undocumented_public_function() -> None:
    """The configured warning policy must fail on an undocumented declaration."""
    completed = subprocess.run(  # noqa: S603 - fixed local TypeDoc fixture.
        [
            "pnpm",
            "exec",
            "typedoc",
            "--options",
            str(FIXTURE_CONFIG),
        ],
        cwd=PROJECT_ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    diagnostics = completed.stdout + completed.stderr
    assert completed.returncode != 0, diagnostics
    assert "undocumentedFixture" in diagnostics
