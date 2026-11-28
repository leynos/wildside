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


def package_name(specifier: str) -> str:
    """Return the package name a Bun specifier names, whatever its version.

    A contract may only need to know whether a package is present, not which
    release resolved, so this reader accepts ordinary, prerelease, non-numeric
    and peer-qualified specifiers alike. Splitting on the last separator is not
    enough: a scoped name begins with `@` and can carry a peer suffix.

    Parameters
    ----------
    specifier : str
        The first element of a package entry, such as `name@1.2.3`.

    Returns
    -------
    str
        The package name without its version.

    Raises
    ------
    ValueError
        If the specifier is empty, not a string, or names no package.

    Examples
    --------
    >>> package_name("extract-zip@2.0.1")
    'extract-zip'
    >>> package_name("@puppeteer/browsers@2.6.1")
    '@puppeteer/browsers'
    >>> package_name("get-stream@5.2.0-beta.1")
    'get-stream'
    >>> package_name("get-stream")
    'get-stream'
    """
    if not isinstance(specifier, str) or not specifier:
        message = "a Bun package entry must name its package"
        raise ValueError(message)
    # A scoped name's leading `@` is not the version separator.
    search_from = 1 if specifier.startswith("@") else 0
    separator = specifier.find("@", search_from)
    name = specifier if separator == -1 else specifier[:separator]
    if not name:
        message = f"a Bun package entry names no package: {specifier!r}"
        raise ValueError(message)
    return name


def resolved_names(packages: PackageTable) -> set[str]:
    """Return the name of every package the table resolves.

    Unlike :func:`resolved_versions`, this keeps entries whose version is not
    numeric, because absence is a question about names alone.

    Parameters
    ----------
    packages : PackageTable
        Bun's package table.

    Returns
    -------
    set[str]
        The package name of every entry.

    Raises
    ------
    ValueError
        If an entry is not a non-empty list whose first element is a string.

    Examples
    --------
    >>> resolved_names({"a": ["a@1.2.3"], "b/a": ["a@2.0.0"]})
    {'a'}
    >>> resolved_names({"a": ["a@latest"]})
    {'a'}
    """
    names = set()
    for entry in packages.values():
        if not isinstance(entry, list) or not entry:
            message = "a Bun package entry must be a non-empty list"
            raise ValueError(message)
        specifier = _specifier_of(entry)
        if specifier is None:
            message = f"a Bun package entry names no package: {entry[0]!r}"
            raise ValueError(message)
        names.add(package_name(specifier))
    return names


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
