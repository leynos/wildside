"""Keep the `minimatch` and `glob` audit exceptions honest.

GHSA-g3hv-75wg-m249 (denial of service through globparts amplification) covers
`minimatch` below 11.0.1, and GHSA-3g3m-2fjr-6pw4 (an AbortSignal cancellation
bypass) covers `glob` below 14.0.2. Neither patched release is reachable here:
`glob` 14 requires `minimatch ^11`, while every consumer in the tree declares
`minimatch` 10.x. The entries argue from reach and from the major bump, so
these tests fail when either half of that argument stops being true, whether in
`pnpm-lock.yaml` or in `bun.lock`. The shared checks over the whole ledger stay
in `audit_exception_test.py`.
"""

from __future__ import annotations

import json
import typing as typ
from pathlib import Path

import bun_lockfile as lock
import pytest
import yaml

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
LEDGER_PATH = REPOSITORY_ROOT / "security" / "audit-exceptions.json"
BUN_LOCK_PATH = REPOSITORY_ROOT / "bun.lock"
PNPM_LOCK_PATH = REPOSITORY_ROOT / "pnpm-lock.yaml"

MINIMATCH_EXCEPTION_ID = "MINIMATCH_GLOBPARTS_AMPLIFICATION_2026_10"
GLOB_EXCEPTION_ID = "GLOB_ABORTSIGNAL_BYPASS_2026_10"
MINIMATCH_GLOB_TRACKING_ISSUE = "https://github.com/leynos/wildside/issues/576"

#: The first release of each package that carries the fix. A resolved version
#: at or above these has the patch, and the matching entry is then unnecessary
#: rather than merely stale.
FIRST_PATCHED_MINIMATCH = (11, 0, 1)
FIRST_PATCHED_GLOB = (14, 0, 2)


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


def _pnpm_resolved_versions(package: str) -> list[lock.Version]:
    """Return the versions of `package` resolved by the pnpm lockfile.

    The audit gate checks both lockfiles, so an exception that is void for one
    is not necessarily void for the other. pnpm records resolutions as
    `<name>@<version>` keys in `snapshots`, which is enough to answer "is it
    still here, and at what version" without a full graph.
    """
    snapshots: dict[str, typ.Any] = yaml.safe_load(
        PNPM_LOCK_PATH.read_text(encoding="utf-8")
    )["snapshots"]
    versions: list[lock.Version] = []
    for key in snapshots:
        name, _, version = key.rpartition("@")
        if name != package:
            continue
        parsed = lock.parse_specifier(f"{name}@{version}")
        if parsed[1] is not None:
            versions.append(parsed[1])
    return sorted(versions)


def _assert_still_necessary(
    exception_id: str,
    package: str,
    first_patched: lock.Version,
    packages: lock.PackageTable,
) -> None:
    """Fail when `package` resolves only patched releases, or none at all.

    An entry that outlives its premise is worse than a missing one: it is a
    repository-wide suppression that no gate is watching, because the argument
    in its reason has quietly stopped being true.
    """
    bun_resolved = lock.resolved_versions(packages, package)
    pnpm_resolved = _pnpm_resolved_versions(package)
    assert bun_resolved or pnpm_resolved, (
        f"{exception_id} still excepts an advisory against {package}, but "
        f"neither bun.lock nor pnpm-lock.yaml resolves it at all. Remove the "
        f"entry and close {MINIMATCH_GLOB_TRACKING_ISSUE}."
    )

    resolved = sorted({*bun_resolved, *pnpm_resolved})
    vulnerable = [version for version in resolved if version < first_patched]
    assert vulnerable, (
        f"{exception_id} excepts an advisory against {package} on the grounds "
        f"that its fix is a major bump the tree cannot take, but bun.lock and "
        f"pnpm-lock.yaml together now resolve only {resolved}, all of which "
        f"carry the fix. Remove the entry and close "
        f"{MINIMATCH_GLOB_TRACKING_ISSUE}."
    )


def test_the_minimatch_exception_is_void_once_a_patched_release_resolves(
    ledger_ids: frozenset[str], packages: lock.PackageTable
) -> None:
    """The entry rests on `minimatch` 11 being out of reach.

    Every consumer in the tree declares `minimatch` 10.x, including the latest
    releases of those consumers, so forcing 11 breaks them. The day the tree
    resolves only 11.0.1 or later, some consumer has caught up and the entry
    is unnecessary.
    """
    if MINIMATCH_EXCEPTION_ID not in ledger_ids:
        pytest.skip("the minimatch advisory is no longer excepted")

    _assert_still_necessary(
        MINIMATCH_EXCEPTION_ID, "minimatch", FIRST_PATCHED_MINIMATCH, packages
    )


def test_the_glob_exception_is_void_once_a_patched_release_resolves(
    ledger_ids: frozenset[str], packages: lock.PackageTable
) -> None:
    """The entry rests on `glob` 14 being unavailable to our consumers.

    `glob` 14 requires `minimatch ^11`, and `style-dictionary` still depends on
    `@bundled-es-modules/glob` 13, so the fix arrives with the same consumer
    upgrade as the minimatch entry. The day the tree resolves only 14.0.2 or
    later, the entry is unnecessary.
    """
    if GLOB_EXCEPTION_ID not in ledger_ids:
        pytest.skip("the glob advisory is no longer excepted")

    _assert_still_necessary(GLOB_EXCEPTION_ID, "glob", FIRST_PATCHED_GLOB, packages)
