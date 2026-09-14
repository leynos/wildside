"""Read the parts of a Bun lockfile that audit-exception contracts argue from.

An exception in `security/audit-exceptions.json` is defensible only while the
dependency tree still looks the way its reason says it does, so the contracts
in this directory ask two questions of `bun.lock`: which versions of a package
it resolves, and which packages reach one. Both are answered here so that the
parsing lives in one place and the contracts read as assertions.
"""

from __future__ import annotations

import json
import re

#: Bun writes JSONC: object literals carry trailing commas that `json` rejects.
_TRAILING_COMMA = re.compile(r",(\s*[}\]])")

#: Package entries are keyed by `name@version` in the lockfile's value tuples.
_SPECIFIER = re.compile(r"^(?P<name>.+)@(?P<major>\d+)\.(?P<minor>\d+)\.(?P<patch>\d+)")

#: The dependency fields a resolved package may reach another package through.
#: `peerDependencies` is excluded: Bun records peers a package asks for, not
#: edges it resolves, so including it would report parents that pull nothing in.
_DEPENDENCY_FIELDS = ("dependencies", "optionalDependencies", "devDependencies")

#: A resolved package's entry: its specifier, then further fields Bun writes.
PackageEntry = list[object]

#: Bun's package table, keyed by the dependency path that needed each package.
PackageTable = dict[str, PackageEntry]

#: A release, as the three numbers a version range is compared against.
Version = tuple[int, int, int]


def parse_packages(text: str) -> PackageTable:
    """Parse a lockfile's resolved package table from its text.

    Examples
    --------
        >>> parse_packages('{"packages": {"a": ["a@1.0.0"],}}')["a"][0]
        'a@1.0.0'
    """
    return json.loads(_TRAILING_COMMA.sub(r"\1", text))["packages"]


def parse_specifier(specifier: str) -> tuple[str, Version | None]:
    """Split a specifier into its package name and numeric version.

    A prerelease or other non-numeric version yields `None` for the version,
    since every comparison here is against a release boundary.

    Examples
    --------
        >>> parse_specifier("@puppeteer/browsers@2.6.1")
        ('@puppeteer/browsers', (2, 6, 1))
    """
    match = _SPECIFIER.match(specifier)
    if match is None:
        return specifier, None
    version = (int(match["major"]), int(match["minor"]), int(match["patch"]))
    return match["name"], version


def _specifier_of(entry: PackageEntry) -> str | None:
    """Return an entry's `name@version` specifier, if it carries one."""
    specifier = entry[0]
    return specifier if isinstance(specifier, str) else None


def _declared_dependencies(entry: PackageEntry) -> set[str]:
    """Return every package name an entry declares as a dependency."""
    metadata = next((item for item in entry if isinstance(item, dict)), {})
    declared: set[str] = set()
    for field in _DEPENDENCY_FIELDS:
        edges = metadata.get(field)
        if isinstance(edges, dict):
            declared.update(key for key in edges if isinstance(key, str))
    return declared


def resolved_versions(packages: PackageTable, package: str) -> list[Version]:
    """Return every version of `package` the table resolves.

    A package can appear more than once: Bun keys a nested resolution by the
    path that needed it, such as `vitest/picomatch`, while the value always
    names the package itself.

    Examples
    --------
        >>> resolved_versions({"a": ["a@1.2.3"]}, "a")
        [(1, 2, 3)]
    """
    versions = []
    for entry in packages.values():
        specifier = _specifier_of(entry)
        if specifier is None:
            continue
        name, version = parse_specifier(specifier)
        if name == package and version is not None:
            versions.append(version)
    return versions


def dependents_of(packages: PackageTable, package: str) -> set[str]:
    """Return every resolved package that declares `package` as a dependency.

    The name comes from the entry's own specifier rather than its key, because
    a key can be a dependency path such as `vitest/picomatch` and a scoped
    package name contains the same separator.

    Examples
    --------
        >>> dependents_of({"a": ["a@1.0.0", "", {"dependencies": {"b": "^1"}}]}, "b")
        {'a'}
    """
    dependents = set()
    for entry in packages.values():
        specifier = _specifier_of(entry)
        if specifier is not None and package in _declared_dependencies(entry):
            dependents.add(parse_specifier(specifier)[0])
    return dependents
