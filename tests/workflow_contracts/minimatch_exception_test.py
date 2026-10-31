"""Keep the `minimatch` audit exception honest.

GHSA-g3hv-75wg-m249 (denial of service through globparts amplification) covers
every `minimatch` release below 11.0.1. `glob` 14 already resolves the patched
copy, but `typedoc` and `markdownlint-cli` still declare the 10.x line, and
`typedoc` fails on 11, so the exception in `security/audit-exceptions.json`
argues from reach: both are development-time tools matching glob patterns from
repository-controlled configuration. These tests fail when either half of that
argument stops being true. The shared checks over the whole ledger stay in
`audit_exception_test.py`.
"""

from __future__ import annotations

import json
import typing as typ
from pathlib import Path

import bun_lockfile as lock
import pytest

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
LEDGER_PATH = REPOSITORY_ROOT / "security" / "audit-exceptions.json"
BUN_LOCK_PATH = REPOSITORY_ROOT / "bun.lock"

MINIMATCH_EXCEPTION_ID = "MINIMATCH_GLOBPARTS_DOS_2026_10"
MINIMATCH_TRACKING_ISSUE = "https://github.com/leynos/wildside/issues/575"

#: The first release that carries the fix. A copy below this is what the entry
#: exists to tolerate.
FIRST_PATCHED_MINIMATCH = (11, 0, 1)

#: The only packages the reasoning permits as parents: `glob` (on the patched
#: copy) and the two documentation and Markdown tools the entry names. A parent
#: outside this set is a new way to reach the code, which the entry's reason
#: does not cover.
MINIMATCH_PERMITTED_DEPENDENTS = frozenset({"glob", "markdownlint-cli", "typedoc"})


@pytest.fixture(scope="module")
def ledger_ids() -> frozenset[str]:
    """Return the identifiers of every entry in the audit exception ledger."""
    ledger: list[dict[str, typ.Any]] = json.loads(
        LEDGER_PATH.read_text(encoding="utf-8")
    )
    return frozenset(entry["id"] for entry in ledger)


@pytest.fixture(scope="module")
def packages() -> lock.PackageTable:
    """Return the package table of the repository's Bun lockfile."""
    return lock.parse_packages(BUN_LOCK_PATH.read_text(encoding="utf-8"))


def test_the_minimatch_exception_is_void_once_only_patched_copies_resolve(
    ledger_ids: frozenset[str], packages: lock.PackageTable
) -> None:
    """The entry rests on a vulnerable `minimatch` still being resolved.

    The day `bun.lock` resolves only releases from 11.0.1 up, whether
    `typedoc` and `markdownlint-cli` moved to 11 or left the tree, the advisory
    no longer applies and the exception only hides future findings.
    """
    if MINIMATCH_EXCEPTION_ID not in ledger_ids:
        pytest.skip("the minimatch advisory is no longer excepted")

    resolved = lock.resolved_versions(packages, "minimatch")
    vulnerable = [version for version in resolved if version < FIRST_PATCHED_MINIMATCH]
    assert vulnerable, (
        f"{MINIMATCH_EXCEPTION_ID} excepts GHSA-g3hv-75wg-m249, but bun.lock "
        f"now resolves only {sorted(resolved)}, all patched. Remove the entry "
        f"and close {MINIMATCH_TRACKING_ISSUE}."
    )


def test_the_minimatch_exception_is_void_once_anything_else_pulls_it(
    ledger_ids: frozenset[str], packages: lock.PackageTable
) -> None:
    """The entry is repository-wide, but its reason names two parents.

    `run-bun-audit.js` turns a ledger entry into a `--ignore` for every caller,
    while the reason argues only that `typedoc` and `markdownlint-cli` reach
    `minimatch` from repository-controlled configuration. A further parent
    would be covered by an argument written about neither.
    """
    if MINIMATCH_EXCEPTION_ID not in ledger_ids:
        pytest.skip("the minimatch advisory is no longer excepted")

    dependents = lock.dependents_of(packages, "minimatch")
    unexpected = dependents - MINIMATCH_PERMITTED_DEPENDENTS
    assert not unexpected, (
        f"{MINIMATCH_EXCEPTION_ID} excepts minimatch on the grounds that only "
        f"{sorted(MINIMATCH_PERMITTED_DEPENDENTS)} reach it, but "
        f"{sorted(unexpected)} now depend on it too. The exception is "
        f"repository-wide, so it would cover those callers."
    )
