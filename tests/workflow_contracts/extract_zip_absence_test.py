"""Keep `extract-zip` out of both dependency graphs.

GHSA-jmr9-qjv8-65gv (unvalidated symlink path traversal) and
GHSA-7pqw-9j4j-h8q3 (arbitrary writes through symlink entries) cover every
published `extract-zip` release up to 2.0.1, and 2.0.1 is the newest release.
No version bump can resolve them, so issue #491 removed the browser-backed
Mermaid tooling instead: `make nixie` and the ER snapshot command both render
through the browser-free `merman-cli`, so no Puppeteer or Mermaid CLI
dependency remains to resolve `extract-zip`.

The repository keeps two lockfiles, so absence has to be asserted in both.
These tests are unconditional: an audit exception would hide a reintroduction
rather than resolve one, so the contract fails `make test` instead.
"""

from __future__ import annotations

import typing as typ
from pathlib import Path

import bun_lockfile as bun
import pnpm_lockfile as pnpm
import pytest
import yaml

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
BUN_LOCK_PATH = REPOSITORY_ROOT / "bun.lock"
PNPM_LOCK_PATH = REPOSITORY_ROOT / "pnpm-lock.yaml"

FORBIDDEN_PACKAGE = "extract-zip"
TRACKING_ISSUE = "https://github.com/leynos/wildside/issues/491"
REMOVAL_INSTRUCTION = (
    f"Remove the dependency that reaches {FORBIDDEN_PACKAGE} and close "
    f"{TRACKING_ISSUE}. Do not add an audit exception: neither advisory has a "
    f"patched release, so an exception only hides the package's return."
)


def bun_resolves(packages: bun.PackageTable) -> bool:
    """Report whether the Bun package table resolves `extract-zip` itself.

    Parameters
    ----------
    packages : bun_lockfile.PackageTable
        Bun's parsed package table.

    Returns
    -------
    bool
        True when an entry names `extract-zip`, whatever its version.

    Examples
    --------
    >>> bun_resolves({"a": ["extract-zip@2.0.1", "", {}]})
    True
    >>> bun_resolves({"extract-zip/get-stream": ["get-stream@5.2.0", "", {}]})
    False
    """
    return FORBIDDEN_PACKAGE in bun.resolved_names(packages)


def bun_dependents(packages: bun.PackageTable) -> set[str]:
    """Return every package the table declares `extract-zip` as a dependency.

    Peer declarations are excluded by `bun_lockfile.dependents_of`: they name
    what a package asks its consumer for, not an edge it pulls in.

    Parameters
    ----------
    packages : bun_lockfile.PackageTable
        Bun's parsed package table.

    Returns
    -------
    set[str]
        The names of packages that depend on `extract-zip`.

    Examples
    --------
    >>> bun_dependents(
    ...     {"p": ["p@1.0.0", "", {"dependencies": {"extract-zip": "^2"}}]}
    ... )
    {'p'}
    """
    return bun.dependents_of(packages, FORBIDDEN_PACKAGE)


def pnpm_resolves(text: str) -> bool:
    r"""Report whether a pnpm lockfile resolves `extract-zip`.

    Parameters
    ----------
    text : str
        The lockfile's text.

    Returns
    -------
    bool
        True when a `packages` or `snapshots` key names `extract-zip`.

    Raises
    ------
    yaml.YAMLError
        If the text is not YAML, or a mapping repeats a key.
    TypeError
        If the document or either package table is malformed.

    Examples
    --------
    >>> pnpm_resolves("packages: {extract-zip@2.0.1: {}}\nsnapshots: {}\n")
    True
    >>> pnpm_resolves("packages: {}\nsnapshots: {}\n")
    False
    """
    return FORBIDDEN_PACKAGE in pnpm.resolved_names(text)


@pytest.fixture(scope="module")
def bun_packages() -> bun.PackageTable:
    """Return the package table of the repository's Bun lockfile."""
    return bun.parse_packages(BUN_LOCK_PATH.read_text(encoding="utf-8"))


@pytest.fixture(scope="module")
def pnpm_lock_text() -> str:
    """Return the text of the repository's pnpm lockfile."""
    return PNPM_LOCK_PATH.read_text(encoding="utf-8")


def test_bun_lock_resolves_no_extract_zip(bun_packages: bun.PackageTable) -> None:
    """A resolved entry is the package's return, whatever version it names."""
    assert not bun_resolves(bun_packages), (
        f"bun.lock resolves {FORBIDDEN_PACKAGE}. {REMOVAL_INSTRUCTION}"
    )


def test_bun_lock_declares_no_dependency_on_extract_zip(
    bun_packages: bun.PackageTable,
) -> None:
    """A dependency edge means a lockfile regeneration could resolve it again.

    Checking only for a resolved entry would miss a parent that declares the
    dependency while the entry itself has already been pruned.
    """
    dependents = bun_dependents(bun_packages)
    assert not dependents, (
        f"bun.lock's {sorted(dependents)} declare {FORBIDDEN_PACKAGE} as a "
        f"dependency. {REMOVAL_INSTRUCTION}"
    )


def test_pnpm_lock_resolves_no_extract_zip(pnpm_lock_text: str) -> None:
    """Both pnpm package tables are searched, so a snapshot alone is caught."""
    assert not pnpm_resolves(pnpm_lock_text), (
        f"pnpm-lock.yaml resolves {FORBIDDEN_PACKAGE}. {REMOVAL_INSTRUCTION}"
    )


@pytest.mark.parametrize(
    "specifier",
    [
        "extract-zip@2.0.1",
        "extract-zip@2.0.2-beta.1",
        "extract-zip@latest",
        "extract-zip@2.0.1(get-stream@5.2.0)",
    ],
)
def test_a_bun_specifier_resolving_the_package_is_detected(specifier: str) -> None:
    """Scenario: a returned package is described with an unusual version.

    Invariant: the name-only reader still reports it, so a prerelease,
    non-numeric or peer-qualified specifier cannot slip past the ban.
    """
    packages: bun.PackageTable = {"some-package/extract-zip": [specifier, "", {}]}
    assert bun_resolves(packages), (
        f"{specifier!r} must be reported as resolving {FORBIDDEN_PACKAGE}"
    )


def test_a_bun_dependency_edge_on_the_package_is_detected() -> None:
    """Scenario: a parent declares the package but no entry resolves it yet."""
    packages: bun.PackageTable = {
        "parent": ["parent@1.0.0", "", {"dependencies": {"extract-zip": "^2"}}]
    }
    assert bun_dependents(packages) == {"parent"}, (
        "a declared dependency edge must name its parent"
    )


def test_a_target_in_only_one_lockfile_is_detected() -> None:
    """Scenario: one graph has the package and the other is clean.

    Invariant: each lockfile is judged on its own evidence, so a clean Bun
    graph cannot excuse a pnpm graph that still resolves the package.
    """
    clean_pnpm = "packages: {a@1.0.0: {}}\nsnapshots: {}\n"
    dirty_bun: bun.PackageTable = {"a": ["extract-zip@2.0.1", "", {}]}

    assert bun_resolves(dirty_bun), "the dirty Bun graph must be detected"
    assert not pnpm_resolves(clean_pnpm), "the clean pnpm graph must not be reported"


@pytest.mark.parametrize(
    ("table", "text"),
    [
        ("the packages table", "packages: {extract-zip@2.0.1: {}}\nsnapshots: {}\n"),
        (
            "the snapshots table, with a peer suffix",
            "packages: {}\nsnapshots: {extract-zip@2.0.1(get-stream@5.2.0): {}}\n",
        ),
    ],
)
def test_a_pnpm_key_resolving_the_package_is_detected(table: str, text: str) -> None:
    """Scenario: either pnpm package table resolves the package.

    Invariant: both tables are searched, so an entry that appears in only one
    of them is still reported.
    """
    assert pnpm_resolves(text), (
        f"a key in {table} must be reported as resolving {FORBIDDEN_PACKAGE}"
    )


def test_a_peer_suffix_mentioning_the_package_is_not_a_resolution() -> None:
    """Scenario: another package's peer suffix names the banned package.

    Invariant: the base name comes from before the peer suffix, so a mention
    in parentheses is not reported as the package itself.
    """
    text = "packages: {other@1.0.0(extract-zip@2.0.1): {}}\nsnapshots: {}\n"
    assert not pnpm_resolves(text), (
        "a peer suffix must not be read as the package itself"
    )


def test_a_bun_peer_declaration_mentioning_the_package_is_clean() -> None:
    """Scenario: a package asks its consumer for the banned package as a peer.

    Invariant: a peer declaration is not a resolved edge, so it neither
    resolves the package nor makes the declaring package its dependent.
    """
    packages: bun.PackageTable = {
        "other": [
            "other@1.0.0",
            "",
            {"peerDependencies": {"extract-zip": "^2"}},
        ]
    }
    assert not bun_resolves(packages), "a peer declaration is not a resolution"
    assert not bun_dependents(packages), "a peer declaration is not an edge"


def test_a_bun_path_naming_the_package_but_resolving_another_is_clean() -> None:
    """Scenario: Bun keys a nested resolution by the path that needed it."""
    packages: bun.PackageTable = {
        "extract-zip/get-stream": ["get-stream@5.2.0", "", {}]
    }
    assert not bun_resolves(packages), "a dependency path is not a resolution"
    assert not bun_dependents(packages), "a dependency path is not an edge"


def test_a_repeated_pnpm_key_is_refused() -> None:
    """Scenario: a lockfile repeats a mapping key.

    Invariant: the strict loader refuses the document rather than reading only
    the value PyYAML kept, which could hide a banned entry.
    """
    text = "packages:\n  a@1.0.0: {}\n  a@1.0.0: {}\nsnapshots: {}\n"
    with pytest.raises(yaml.YAMLError):
        pnpm_resolves(text)


@pytest.mark.parametrize(
    "text",
    [
        "packages: []\nsnapshots: {}\n",
        "packages: {}\nsnapshots: nope\n",
        "packages: {}\n",
        "just a string\n",
    ],
)
def test_malformed_pnpm_tables_are_refused(text: str) -> None:
    """Scenario: a package table is missing or has the wrong shape.

    Invariant: the reader raises instead of returning an empty set, because an
    empty set would read as a clean graph.
    """
    with pytest.raises(TypeError, match="a mapping"):
        pnpm_resolves(text)


@pytest.mark.parametrize(
    "entry",
    [
        [],
        "extract-zip@2.0.1",
        [""],
        [123],
    ],
)
def test_malformed_bun_entries_are_refused(entry: object) -> None:
    """Scenario: a Bun package entry cannot name a package.

    Invariant: the reader raises instead of silently skipping the entry, since
    skipping is exactly how a banned package would escape the check.
    """
    packages = typ.cast("bun.PackageTable", {"broken": entry})
    with pytest.raises(ValueError, match="package"):
        bun.resolved_names(packages)
