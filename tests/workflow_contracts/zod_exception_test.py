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

Every file is opened by a fixture through `repository_reading`, so a missing or
malformed file fails there with its path. The queries below take what the
fixtures parsed and open nothing.
"""

from __future__ import annotations

import typing as typ
from pathlib import Path

import bun_lockfile as lock
import pytest
from repository_reading import RepositoryReadError, parse_json, read_text

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

#: Every manifest field through which a workspace can declare a dependency,
#: optional ones included: a consumer that lists zod only as an optional
#: dependency still parses with it when it is installed.
DEPENDENCY_FIELDS = (
    "dependencies",
    "devDependencies",
    "peerDependencies",
    "optionalDependencies",
)


class LedgerEntry(typ.TypedDict):
    """The part of an audit exception entry this contract reads."""

    id: str


#: A manifest narrowed to its dependency tables: field name to package name to
#: version range. Fields a manifest does not declare are absent.
Manifest = dict[str, dict[str, str]]


def _read_json_object(path: Path) -> dict[str, object]:
    """Return the JSON object a repository file holds.

    Parameters
    ----------
    path : Path
        The file to read.

    Returns
    -------
    dict[str, object]
        The file's top-level object.

    Raises
    ------
    RepositoryReadError
        If the file cannot be read, is not JSON, or is not an object.
    """
    value = parse_json(read_text(path), path)
    if not isinstance(value, dict):
        message = f"{path}: must hold a JSON object"
        raise RepositoryReadError(message, path=path)
    return value


def _manifest_of(path: Path) -> Manifest:
    """Return one package manifest narrowed to its dependency tables.

    Parameters
    ----------
    path : Path
        The `package.json` to read.

    Returns
    -------
    Manifest
        Only the dependency fields the manifest declares, each a table of
        package name to range.

    Raises
    ------
    RepositoryReadError
        If the manifest cannot be read or a dependency field is not a table.
    """
    data = _read_json_object(path)
    manifest: Manifest = {}
    for field in DEPENDENCY_FIELDS:
        table = data.get(field)
        if table is None:
            continue
        if not isinstance(table, dict):
            message = f"{path}: {field} must be an object"
            raise RepositoryReadError(message, path=path)
        manifest[field] = {str(name): str(rng) for name, rng in table.items()}
    return manifest


def _workspace_directories(root: Path) -> list[Path]:
    """Return the repository root and every workspace directory below it.

    The directory under `packages` is listed with `iterdir` rather than
    matched with `glob`, because `glob` reports a missing directory as an empty
    result, and an empty result would read as a workspace set that declares
    nothing.

    Parameters
    ----------
    root : Path
        The repository root.

    Returns
    -------
    list[Path]
        The root, `frontend-pwa` and each directory under `packages`, by name.

    Raises
    ------
    RepositoryReadError
        If `packages` cannot be listed.
    """
    packages = root / "packages"
    try:
        children = sorted(entry for entry in packages.iterdir() if entry.is_dir())
    except OSError as error:
        message = f"{packages}: cannot be listed: {error}"
        raise RepositoryReadError(message, path=packages) from error
    return [root, root / "frontend-pwa", *children]


def workspaces_declaring_zod(manifests: dict[str, Manifest]) -> frozenset[str]:
    """Return the workspaces whose manifest declares zod in any field.

    Parameters
    ----------
    manifests : dict[str, Manifest]
        Workspace directory, relative to the root, to its parsed manifest.

    Returns
    -------
    frozenset[str]
        The workspace directories that declare zod.

    Examples
    --------
    >>> workspaces_declaring_zod({"a": {"dependencies": {"zod": "^3"}}, "b": {}})
    frozenset({'a'})

    An optional dependency counts:

    >>> workspaces_declaring_zod({"a": {"optionalDependencies": {"zod": "^3"}}})
    frozenset({'a'})
    """
    return frozenset(
        workspace
        for workspace, manifest in manifests.items()
        if any("zod" in manifest.get(field, {}) for field in DEPENDENCY_FIELDS)
    )


@pytest.fixture(scope="module")
def ledger_ids() -> frozenset[str]:
    """Return the identifiers of every entry in the audit exception ledger."""
    value = parse_json(read_text(LEDGER_PATH), LEDGER_PATH)
    if not isinstance(value, list):
        message = f"{LEDGER_PATH}: must hold a JSON array"
        raise RepositoryReadError(message, path=LEDGER_PATH)
    entries: list[LedgerEntry] = typ.cast("list[LedgerEntry]", value)
    return frozenset(entry["id"] for entry in entries)


@pytest.fixture(scope="module")
def packages() -> lock.PackageTable:
    """Return the package table of the repository's Bun lockfile."""
    return lock.parse_packages(read_text(BUN_LOCK_PATH))


@pytest.fixture(scope="module")
def workspace_manifests() -> dict[str, Manifest]:
    """Return every workspace manifest, keyed by directory from the root."""
    return {
        directory.relative_to(REPOSITORY_ROOT).as_posix() or ".": _manifest_of(
            directory / "package.json"
        )
        for directory in _workspace_directories(REPOSITORY_ROOT)
    }


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
    ledger_ids: frozenset[str], workspace_manifests: dict[str, Manifest]
) -> None:
    """The workspace's own use of zod is argued about, so it is pinned too.

    `bun.lock`'s package table never lists workspace packages, so the check
    above cannot see a new workspace that declares zod. This one reads the
    manifests, and also fails if a named workspace stops declaring zod, which
    is the migration's first sign and means the argument needs rewriting.
    """
    if ZOD_EXCEPTION_ID not in ledger_ids:
        pytest.skip("the zod advisory is no longer excepted")

    declaring = workspaces_declaring_zod(workspace_manifests)
    assert declaring == ZOD_PERMITTED_WORKSPACES, (
        f"{ZOD_EXCEPTION_ID} argues from {sorted(ZOD_PERMITTED_WORKSPACES)} "
        f"being the workspaces that declare zod, but {sorted(declaring)} do. "
        f"Rewrite the entry's reason or remove it, and see {ZOD_TRACKING_ISSUE}."
    )


def test_an_optional_dependency_on_zod_is_a_declaration(
    tmp_path: Path,
) -> None:
    """Scenario: a new workspace lists zod only under `optionalDependencies`.

    Invariant: the manifest reader keeps that field, and the query reports the
    workspace, so the removal test above cannot be bypassed by choosing it.
    """
    manifest_path = tmp_path / "package.json"
    manifest_path.write_text(
        '{"optionalDependencies": {"zod": "^3"}}', encoding="utf-8"
    )
    manifests = {"packages/extra": _manifest_of(manifest_path)}
    assert workspaces_declaring_zod(manifests) == frozenset({"packages/extra"})


def test_a_manifest_that_is_not_json_names_the_file(tmp_path: Path) -> None:
    """Scenario: a workspace manifest is truncated.

    Invariant: the failure names that manifest, not a key several frames later.
    """
    manifest_path = tmp_path / "package.json"
    manifest_path.write_text('{"dependencies": ', encoding="utf-8")
    with pytest.raises(RepositoryReadError, match=r"package\.json"):
        _manifest_of(manifest_path)
