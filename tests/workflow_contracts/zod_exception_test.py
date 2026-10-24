"""Keep the `zod` audit exception honest.

GHSA-mrhx-c7xc-m3w5 (memory exhaustion validating a large unconstrained array)
is patched only in zod 4.6.6 and above. The workspace is on zod 3, so the
exception in `security/audit-exceptions.json` is a fourteen-day stopgap for a
major-version migration, argued from reach, and reach has two halves that are
read from different places. The registry packages that depend on zod are read
from the Bun lockfile's package table, which lists resolved registry packages
only: it holds `chromium-bidi` (Puppeteer's build-time tooling) and never the
workspace's own packages. The workspace's own use is therefore read separately,
from the workspace manifests: `frontend-pwa` and `packages/types` declare zod
directly, for `UsersSchema` and the fetcher. These tests fail when the
migration lands or when either half gains a parent the reason does not cover.
The shared checks over the whole ledger stay in `audit_exception_test.py`.
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

#: The only resolved registry package the reasoning permits as a parent:
#: Puppeteer's tooling, which never parses untrusted input. A parent outside
#: this set is a new way to reach the code, which the entry's reason does not
#: cover. Workspace packages are not in the lockfile's package table, so they
#: are checked against the next constant instead.
ZOD_PERMITTED_DEPENDENTS = frozenset({"chromium-bidi"})

#: The workspace manifests, relative to the repository root, that the reason
#: names as declaring zod: `frontend-pwa` parses API responses through it and
#: `packages/types` defines the schemas. A third manifest declaring zod is a new
#: consumer the argument about `UsersSchema` was not written for.
ZOD_PERMITTED_WORKSPACES = frozenset({"frontend-pwa", "packages/types"})


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


def _workspaces_declaring_zod() -> frozenset[str]:
    """Return the workspace directories whose manifest declares zod.

    Examples
    --------
        >>> isinstance(_workspaces_declaring_zod(), frozenset)
        True
    """
    manifests = [
        REPOSITORY_ROOT / "package.json",
        *sorted(REPOSITORY_ROOT.glob("frontend-pwa/package.json")),
        *sorted(REPOSITORY_ROOT.glob("packages/*/package.json")),
    ]
    declaring = set()
    for manifest in manifests:
        data: dict[str, typ.Any] = json.loads(manifest.read_text(encoding="utf-8"))
        fields = ("dependencies", "devDependencies", "peerDependencies")
        if any("zod" in data.get(field, {}) for field in fields):
            declaring.add(manifest.parent.relative_to(REPOSITORY_ROOT).as_posix())
    return frozenset(declaring)


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


def test_the_zod_exception_is_void_once_another_workspace_declares_zod(
    ledger_ids: frozenset[str],
) -> None:
    """The workspace's own use of zod is argued about, so it is pinned too.

    `bun.lock`'s package table never lists workspace packages, so the check
    above cannot see a new workspace that declares zod. This one reads the
    manifests, and also fails if a named workspace stops declaring zod, which
    is the migration's first sign and means the argument needs rewriting.
    """
    if ZOD_EXCEPTION_ID not in ledger_ids:
        pytest.skip("the zod advisory is no longer excepted")

    declaring = _workspaces_declaring_zod()
    assert declaring == ZOD_PERMITTED_WORKSPACES, (
        f"{ZOD_EXCEPTION_ID} argues from {sorted(ZOD_PERMITTED_WORKSPACES)} "
        f"being the workspaces that declare zod, but {sorted(declaring)} do. "
        f"Rewrite the entry's reason or remove it, and see {ZOD_TRACKING_ISSUE}."
    )
