"""Property tests for the lockfile name readers the absence contracts use.

The example tables in :mod:`extract_zip_absence_test` and
:mod:`zod_exception_test` pin the readers against the lockfiles this repository
happens to carry today. These tests pin the readers' own rule instead: a
package name survives whatever version, prerelease tag, peer suffix or
dependency path a lockfile attaches to it, because the absence contracts only
ask whether a name is present.

Every strategy builds the shape it means to exercise, so the search budget is
spent on the separator boundary rather than on discarding generated text.
"""

from __future__ import annotations

import typing as typ

import bun_lockfile as bun
import pnpm_lockfile as pnpm
import pytest
from hypothesis import given
from hypothesis import strategies as st

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: Characters that may follow the first one in a generated package name.
_NAME_CHARS = "abcdefghijklmnopqrstuvwxyz0123456789._-"

#: A name's first character. Excluding the separators keeps a generated name
#: from looking like the `@` of a scope or the `@` before a version.
_INITIAL_CHARS = "abcdefghijklmnopqrstuvwxyz0123456789"

#: A prerelease tag: letters and digits only, so it can never carry the `@`
#: that separates a name from its version.
_TAG_CHARS = "abcdefghijklmnopqrstuvwxyz0123456789"

_segments = st.text(alphabet=_NAME_CHARS, min_size=1).filter(
    lambda segment: segment[0] in _INITIAL_CHARS
)

plain_names = _segments
scoped_names = st.builds(
    lambda scope, package: f"@{scope}/{package}", scope=_segments, package=_segments
)

#: Names of either shape npm allows: bare, or scoped under an `@scope/`.
names = st.one_of(plain_names, scoped_names)

versions = st.builds(
    lambda major, minor, patch: f"{major}.{minor}.{patch}",
    major=st.integers(min_value=0, max_value=99),
    minor=st.integers(min_value=0, max_value=99),
    patch=st.integers(min_value=0, max_value=99),
)

prerelease_tags = st.text(alphabet=_TAG_CHARS, min_size=1)


@given(names, versions)
def test_a_bun_specifier_names_its_package(name: str, version: str) -> None:
    """A resolved release keeps its name whatever the version numbers."""
    assert bun.package_name(f"{name}@{version}") == name


@given(names, versions, prerelease_tags)
def test_a_bun_prerelease_specifier_names_its_package(
    name: str, version: str, tag: str
) -> None:
    """Bun may resolve a prerelease, which is still the same package."""
    assert bun.package_name(f"{name}@{version}-{tag}") == name


@given(plain_names)
def test_a_bare_bun_name_is_its_own_package(name: str) -> None:
    """An entry without a version separator names the package itself."""
    assert bun.package_name(name) == name


@given(names, versions)
def test_a_pnpm_key_names_its_package(name: str, version: str) -> None:
    """A `packages` or `snapshots` key strips the version it carries."""
    assert pnpm.base_name(f"{name}@{version}") == name


@given(names, versions, names)
def test_a_pnpm_peer_suffix_is_not_part_of_the_package_name(
    name: str, version: str, peer: str
) -> None:
    """A peer release in parentheses names another package, not this one."""
    assert pnpm.base_name(f"{name}@{version}({peer}@1.0.0)") == name


@given(names, versions)
def test_bun_resolved_names_reports_every_generated_name(
    name: str, version: str
) -> None:
    """The table's keys are dependency paths, so only entries name packages."""
    packages: bun.PackageTable = {f"parent/{name}": [f"{name}@{version}"]}
    assert bun.resolved_names(packages) == {name}


@given(names, versions)
def test_pnpm_package_names_reads_both_tables(name: str, version: str) -> None:
    """Either table alone is enough for a package to count as resolved."""
    key = f"{name}@{version}"
    assert pnpm.package_names({"packages": {key: {}}, "snapshots": {}}) == {name}
    assert pnpm.package_names({"packages": {}, "snapshots": {key: {}}}) == {name}


@given(names, versions)
def test_pnpm_package_names_reports_a_package_once(name: str, version: str) -> None:
    """The same package in both tables is one resolution, not two."""
    key = f"{name}@{version}"
    assert pnpm.package_names({"packages": {key: {}}, "snapshots": {key: {}}}) == {name}


@pytest.mark.parametrize("reader", [bun.package_name, pnpm.base_name])
def test_an_empty_key_is_refused(reader: cabc.Callable[[str], str]) -> None:
    """An empty key names nothing, so the reader cannot answer about it."""
    with pytest.raises(ValueError, match="package"):
        reader("")
