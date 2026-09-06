"""Contract tests for the zero-tolerance TypeDoc documentation gate."""

from __future__ import annotations

import json
import re
import subprocess
from pathlib import Path

import pytest

PROJECT_ROOT = Path(__file__).resolve().parents[2]
PACKAGE_MANIFEST = PROJECT_ROOT / "package.json"
MAKEFILE = PROJECT_ROOT / "Makefile"
PNPM_LOCK = PROJECT_ROOT / "pnpm-lock.yaml"
BUN_LOCK = PROJECT_ROOT / "bun.lock"
FIXTURE_CONFIG = Path(__file__).parent / "fixtures" / "typedoc.json"

TYPEDOC_CONFIGS = (
    "frontend-pwa/typedoc.json",
    "packages/types/typedoc.json",
    "packages/tokens/typedoc.json",
)
EXACT_VERSION = re.compile(r"^\d+\.\d+\.\d+$")

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


def _typedoc_pin() -> str:
    """Return the root manifest's declared TypeDoc version."""
    manifest = json.loads(PACKAGE_MANIFEST.read_text(encoding="utf-8"))
    return manifest["devDependencies"]["typedoc"]


def test_typedoc_is_pinned_to_an_exact_version() -> None:
    """A range would let a TypeDoc minor change the gate's verdict silently."""
    pin = _typedoc_pin()
    assert EXACT_VERSION.fullmatch(pin), (
        f"typedoc must be pinned to an exact version, found {pin!r}"
    )


@pytest.mark.parametrize("lockfile", [PNPM_LOCK, BUN_LOCK], ids=["pnpm", "bun"])
def test_lockfiles_resolve_the_pinned_typedoc(lockfile: Path) -> None:
    """Both lockfiles must resolve TypeDoc to the version the manifest names."""
    pin = _typedoc_pin()
    assert f"typedoc@{pin}" in lockfile.read_text(encoding="utf-8"), (
        f"{lockfile.name} does not resolve typedoc@{pin}"
    )


@pytest.mark.parametrize("config_path", TYPEDOC_CONFIGS)
def test_every_surface_enforces_the_zero_tolerance_policy(config_path: str) -> None:
    """Each surface must fail on an undocumented export and emit nothing.

    The fixture test proves TypeDoc honours this policy. This test proves the
    three real configurations actually set it, so a surface cannot quietly
    opt out by dropping a key.
    """
    config = json.loads((PROJECT_ROOT / config_path).read_text(encoding="utf-8"))
    assert config["validation"]["notDocumented"] is True
    assert config["treatValidationWarningsAsErrors"] is True
    assert config["emit"] == "none"
    assert config["requiredToBeDocumented"], (
        "an empty requiredToBeDocumented list disables the gate"
    )
