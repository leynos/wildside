"""Keep the `zod` audit exception honest.

GHSA-mrhx-c7xc-m3w5 (memory exhaustion validating a large unconstrained array)
is patched only in zod 4.6.6 and above. The workspace is on zod 3, so the
exception in `security/audit-exceptions.json` is a fourteen-day stopgap for a
major-version migration, argued from reach: the only dependent in the Bun
lockfile is `chromium-bidi`, which is Puppeteer's build-time tooling, while the
workspace's own use is declared in `frontend-pwa` and `packages/types`. These
tests fail when the migration lands or when a new package starts depending on
zod, either of which retires the argument. The shared checks over the whole
ledger stay in `audit_exception_test.py`.
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

ZOD_EXCEPTION_ID = "ZOD_ARRAY_MEMORY_EXHAUSTION_2026_10"
ZOD_TRACKING_ISSUE = "https://github.com/leynos/wildside/issues/561"

#: The first release that carries the fix. A resolved version below this is
#: what the exception exists to tolerate.
FIRST_PATCHED_ZOD = (4, 6, 6)

#: The only resolved package the reasoning permits as a parent: Puppeteer's
#: tooling, which never parses untrusted input. A parent outside this set is a
#: new way to reach the code, which the entry's reason does not cover.
ZOD_PERMITTED_DEPENDENTS = frozenset({"chromium-bidi"})


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


def test_the_zod_exception_is_void_once_only_patched_releases_resolve(
    ledger_ids: frozenset[str], packages: lock.PackageTable
) -> None:
    """The exception rests on the fix being a major-version migration away.

    The day `bun.lock` resolves only zod 4.6.6 or later, the migration has
    landed and the advisory no longer applies, so the entry is unnecessary.
    """
    if ZOD_EXCEPTION_ID not in ledger_ids:
        pytest.skip("the zod advisory is no longer excepted")

    resolved = lock.resolved_versions(packages, "zod")
    assert resolved, "bun.lock must resolve zod for this entry to apply"

    vulnerable = [version for version in resolved if version < FIRST_PATCHED_ZOD]
    assert vulnerable, (
        f"{ZOD_EXCEPTION_ID} excepts GHSA-mrhx-c7xc-m3w5 on the grounds that the "
        f"fix needs a major-version migration, but bun.lock now resolves only "
        f"{sorted(resolved)}. Remove the entry and close {ZOD_TRACKING_ISSUE}."
    )


def test_the_zod_exception_is_void_once_anything_else_pulls_it(
    ledger_ids: frozenset[str], packages: lock.PackageTable
) -> None:
    """The entry is repository-wide, but its reason is about one parent.

    `run-bun-audit.js` turns a ledger entry into a `--ignore` for every caller,
    while the reason argues only that Puppeteer's `chromium-bidi` reaches zod
    through tooling. A further parent would be covered by an argument written
    about neither, and checking that zod is still resolved would not notice.
    """
    if ZOD_EXCEPTION_ID not in ledger_ids:
        pytest.skip("the zod advisory is no longer excepted")

    dependents = lock.dependents_of(packages, "zod")
    unexpected = dependents - ZOD_PERMITTED_DEPENDENTS
    assert not unexpected, (
        f"{ZOD_EXCEPTION_ID} excepts zod on the grounds that only "
        f"{sorted(ZOD_PERMITTED_DEPENDENTS)} reach it, but "
        f"{sorted(unexpected)} now depend on it too. The exception is "
        f"repository-wide, so it would cover those callers."
    )
