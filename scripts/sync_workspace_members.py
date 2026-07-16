#!/usr/bin/env python3
"""Keep Cargo workspace members in sync with the repository layout."""

from __future__ import annotations

import sys
from pathlib import Path

try:  # Python >=3.11
    import tomllib  # type: ignore[attr-defined]
except ModuleNotFoundError:  # pragma: no cover - fallback for older Python
    import tomli as tomllib  # type: ignore[no-redef]

ROOT = Path(__file__).resolve().parent.parent
MANIFEST = ROOT / "Cargo.toml"


def read_patterns() -> list[str]:
    """Return the workspace autodiscover glob patterns from Cargo.toml.

    Returns
    -------
    list of str
        Configured autodiscover globs, or an empty list if none are set.
    """
    data = tomllib.loads(MANIFEST.read_text(encoding="utf-8"))
    workspace = data.get("workspace", {})
    metadata = workspace.get("metadata", {})
    autodiscover = metadata.get("autodiscover", {})
    globs = autodiscover.get("globs", [])
    if not isinstance(globs, list):
        return []
    return [str(pattern) for pattern in globs]


def discover_members(globs: list[str]) -> list[str]:
    """Return workspace-relative paths of crates matching *globs*.

    Parameters
    ----------
    globs : list of str
        Glob patterns, resolved relative to the repository root.

    Returns
    -------
    list of str
        POSIX-style paths, relative to the repository root, of directories
        matching *globs* that contain a ``Cargo.toml`` file.
    """
    members: list[str] = []
    for pattern in globs:
        for path in sorted(ROOT.glob(pattern)):
            if not path.is_dir():
                continue
            if (path / "Cargo.toml").is_file():
                members.append(path.relative_to(ROOT).as_posix())
    return members


def unique_preserving_order(items: list[str]) -> list[str]:
    """Return *items* with duplicates removed, preserving first occurrence.

    Parameters
    ----------
    items : list of str
        Values to deduplicate.

    Returns
    -------
    list of str
        *items* in original order, with later duplicates dropped.

    Examples
    --------
    >>> unique_preserving_order(["a", "b", "a", "c"])
    ['a', 'b', 'c']
    """
    seen: set[str] = set()
    result: list[str] = []
    for item in items:
        if item in seen:
            continue
        seen.add(item)
        result.append(item)
    return result


def format_members(members: list[str], indent: str) -> list[str]:
    """Render *members* as TOML lines for the workspace ``members`` array.

    Parameters
    ----------
    members : list of str
        Workspace member paths to render.
    indent : str
        Leading whitespace to apply to each rendered line.

    Returns
    -------
    list of str
        Lines forming a TOML ``members = [...]`` array, single-line when
        there is exactly one member and multi-line otherwise.
    """
    if len(members) == 1:
        return [f'{indent}members = ["{members[0]}"]']
    lines = [f"{indent}members = ["]
    lines.extend(f'{indent}    "{member}",' for member in members)
    lines.append(f"{indent}]")
    return lines


def _calculate_bracket_depth_change(line: str) -> int:
    """Compute the net bracket depth delta for a line.

    Parameters
    ----------
    line : str
        A single line of text from the manifest.

    Returns
    -------
    int
        Net change in bracket nesting produced by the line.

    Examples
    --------
    >>> _calculate_bracket_depth_change('members = [')
    1
    >>> _calculate_bracket_depth_change('    ]')
    -1
    """
    return line.count("[") - line.count("]")


def _find_members_array_bounds(lines: list[str]) -> tuple[int, int, str]:
    """Locate the bounds of the workspace members array.

    Parameters
    ----------
    lines : list of str
        Lines from the workspace manifest.

    Returns
    -------
    tuple of int and str
        Start index, end index, and indentation for the members array.

    Raises
    ------
    SystemExit
        If the members array cannot be located in the manifest.

    Examples
    --------
    >>> example = ['[workspace]', 'members = [', '    "crate",', ']']
    >>> _find_members_array_bounds(example)
    (1, 3, '')
    """
    start = None
    indent = ""
    depth = 0
    for idx, line in enumerate(lines):
        stripped = line.lstrip()
        if start is None:
            if not stripped.startswith("members"):
                continue
            start = idx
            indent = line[: len(line) - len(stripped)]
            depth = _calculate_bracket_depth_change(line)
            if depth <= 0:
                return start, idx, indent
            continue
        depth += _calculate_bracket_depth_change(line)
        if depth <= 0:
            return start, idx, indent
    message = "workspace members array not found in Cargo.toml"
    raise SystemExit(message)


def update_manifest(members: list[str]) -> bool:
    """Rewrite the workspace ``members`` array in Cargo.toml if it changed.

    Parameters
    ----------
    members : list of str
        Desired workspace member paths, in order.

    Returns
    -------
    bool
        ``True`` if the manifest was rewritten, ``False`` if it already
        matched *members*.
    """
    lines = MANIFEST.read_text(encoding="utf-8").splitlines()
    start, end, indent = _find_members_array_bounds(lines)
    replacement = format_members(members, indent)
    if lines[start : end + 1] == replacement:
        return False

    lines[start : end + 1] = replacement
    MANIFEST.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return True


def main() -> int:
    """Discover Cargo workspace members and rewrite the manifest if stale.

    Returns
    -------
    int
        Process exit code; always ``0``.
    """
    patterns = read_patterns()
    discovered = discover_members(patterns)
    ordered = unique_preserving_order(["backend", *discovered])
    changed = update_manifest(ordered)
    if changed:
        print("Updated workspace members:", ", ".join(ordered))
    return 0


if __name__ == "__main__":
    sys.exit(main())
