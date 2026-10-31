"""Keep the `deepmerge` audit exception honest.

GHSA-gcqx-w48m-w2h4 (prototype poisoning of the object `mergeObject` returns)
covers every `deepmerge` release from 4.2.0 to 4.3.1, and 4.3.1 is the latest
published, so the exception in `security/audit-exceptions.json` argues from
reach: only `style-dictionary` pulls `deepmerge` in, through
`@bundled-es-modules/deepmerge`, and `packages/tokens` runs it at build time
over repository-controlled token definitions. These tests fail when either half
of that argument stops being true. The shared checks over the whole ledger stay
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

DEEPMERGE_EXCEPTION_ID = "DEEPMERGE_PROTOTYPE_POISONING_2026_10"
DEEPMERGE_TRACKING_ISSUE = "https://github.com/leynos/wildside/issues/570"

#: The last release the advisory covers. A resolved version above this has the
#: fix, and the entry is then unnecessary rather than merely stale.
LAST_VULNERABLE_DEEPMERGE = (4, 3, 1)

#: The only packages the reasoning permits as parents: Style Dictionary's
#: bundled copy, which `packages/tokens` drives from build scripts. A parent
#: outside this set is a new way to reach the code, which the entry's reason
#: does not cover.
DEEPMERGE_PERMITTED_DEPENDENTS = frozenset({"@bundled-es-modules/deepmerge"})


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


def test_the_deepmerge_exception_is_void_once_a_patched_release_resolves(
    ledger_ids: frozenset[str], packages: lock.PackageTable
) -> None:
    """The entry rests on there being nothing patched to move to.

    No release above 4.3.1 exists, so the entry argues from reach instead of
    from a fix. The day both `bun.lock` and `pnpm-lock.yaml` resolve only a
    release above 4.3.1 -- whether `deepmerge` published a patch that Style
    Dictionary adopted or the dependency left both trees -- the advisory no
    longer applies.
    """
    if DEEPMERGE_EXCEPTION_ID not in ledger_ids:
        pytest.skip("the deepmerge advisory is no longer excepted")

    bun_resolved = lock.resolved_versions(packages, "deepmerge")
    pnpm_resolved = _pnpm_resolved_versions("deepmerge")
    assert bun_resolved or pnpm_resolved, (
        f"{DEEPMERGE_EXCEPTION_ID} still excepts GHSA-gcqx-w48m-w2h4, but "
        f"neither bun.lock nor pnpm-lock.yaml resolves deepmerge at all. The "
        f"entry is repository-wide while its reason argues from reach, so it "
        f"now ignores the advisory for every caller. Remove the entry and "
        f"close {DEEPMERGE_TRACKING_ISSUE}."
    )

    # The entry exists because the advisory is reported for both lockfiles, so
    # it stays necessary while *either* tree still resolves a vulnerable
    # release. Demanding removal as soon as one tree drops it would strip
    # suppression from the tree that still needs it.
    resolved = sorted({*bun_resolved, *pnpm_resolved})
    vulnerable = [
        version for version in resolved if version <= LAST_VULNERABLE_DEEPMERGE
    ]
    assert vulnerable, (
        f"{DEEPMERGE_EXCEPTION_ID} excepts GHSA-gcqx-w48m-w2h4 on the grounds "
        f"that no patched deepmerge exists, but bun.lock and pnpm-lock.yaml "
        f"together now resolve only {sorted(resolved)}. Remove the entry and close "
        f"{DEEPMERGE_TRACKING_ISSUE}."
    )


def test_the_deepmerge_exception_is_void_once_anything_else_pulls_it(
    ledger_ids: frozenset[str], packages: lock.PackageTable
) -> None:
    """The entry is repository-wide, but its reason is about one parent.

    `run-bun-audit.js` turns a ledger entry into a `--ignore` for every caller,
    while the reason argues only that Style Dictionary reaches `deepmerge` from
    `packages/tokens` build scripts over repository-controlled definitions. A
    second parent would be covered by an argument written about neither, and
    checking that `deepmerge` is still resolved would not notice.
    """
    if DEEPMERGE_EXCEPTION_ID not in ledger_ids:
        pytest.skip("the deepmerge advisory is no longer excepted")

    dependents = lock.dependents_of(packages, "deepmerge")
    unexpected = dependents - DEEPMERGE_PERMITTED_DEPENDENTS
    assert not unexpected, (
        f"{DEEPMERGE_EXCEPTION_ID} excepts deepmerge on the grounds that only "
        f"{sorted(DEEPMERGE_PERMITTED_DEPENDENTS)} reach it, but "
        f"{sorted(unexpected)} now depend on it too. The exception is "
        f"repository-wide, so it would cover those callers."
    )
