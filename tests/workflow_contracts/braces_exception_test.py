"""Keep the `braces` audit exception honest.

GHSA-vfj7-8cjw-p6xm (stack exhaustion in `braces` at or below 3.0.3) has no
patched release, so the exception in `security/audit-exceptions.json` argues
from reach: only development tooling, through `chokidar` and `micromatch`,
pulls `braces` in, and the patterns it expands come from repository
configuration. These tests fail when either half of that argument stops being
true. The shared checks over the whole ledger stay in `audit_exception_test.py`.
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

BRACES_EXCEPTION_ID = "BRACES_NESTED_PATTERN_DOS_2026_10"
BRACES_TRACKING_ISSUE = "https://github.com/leynos/wildside/issues/527"

#: Releases the advisory covers. A resolved version above this has the fix.
LAST_VULNERABLE_BRACES = (3, 0, 3)

#: The only packages the reasoning permits as parents: file-watching and
#: glob-matching tooling that expands patterns from repository configuration. A
#: parent outside this set is a new way to reach the code, which the entry's
#: reason does not cover.
BRACES_PERMITTED_DEPENDENTS = frozenset({"chokidar", "micromatch"})


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


def test_the_braces_exception_is_void_once_a_patched_release_resolves(
    ledger_ids: frozenset[str], packages: lock.PackageTable
) -> None:
    """The `braces` exception rests on there being nothing patched to move to.

    No release above 3.0.3 exists, so the entry argues from reach instead of
    from a fix. The day `bun.lock` resolves only a release above 3.0.3, whether
    the tooling moved to a patched `braces` or dropped version 3, the advisory
    no longer applies and the entry is unnecessary.
    """
    if BRACES_EXCEPTION_ID not in ledger_ids:
        pytest.skip("the braces advisory is no longer excepted")

    resolved = lock.resolved_versions(packages, "braces")
    assert resolved, "bun.lock must resolve braces for this entry to apply"

    vulnerable = [version for version in resolved if version <= LAST_VULNERABLE_BRACES]
    assert vulnerable, (
        f"{BRACES_EXCEPTION_ID} excepts GHSA-vfj7-8cjw-p6xm on the grounds that no "
        f"patched braces exists, but bun.lock now resolves only "
        f"{sorted(resolved)}. Remove the entry and close {BRACES_TRACKING_ISSUE}."
    )


def test_the_braces_exception_is_void_once_anything_else_pulls_it(
    ledger_ids: frozenset[str], packages: lock.PackageTable
) -> None:
    """The entry is repository-wide, but its reason is about two parents.

    `run-bun-audit.js` turns a ledger entry into a `--ignore` for every caller,
    while the reason argues only that `chokidar` and `micromatch` reach `braces`
    from development tooling. A third parent would be covered by an argument
    written about neither, and checking that `braces` is still resolved would
    not notice.
    """
    if BRACES_EXCEPTION_ID not in ledger_ids:
        pytest.skip("the braces advisory is no longer excepted")

    dependents = lock.dependents_of(packages, "braces")
    unexpected = dependents - BRACES_PERMITTED_DEPENDENTS
    assert not unexpected, (
        f"{BRACES_EXCEPTION_ID} excepts braces on the grounds that only "
        f"{sorted(BRACES_PERMITTED_DEPENDENTS)} reach it, but "
        f"{sorted(unexpected)} now depend on it too. The exception is "
        f"repository-wide, so it would cover those callers."
    )
