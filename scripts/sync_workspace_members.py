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
    data = tomllib.loads(MANIFEST.read_text(encoding="utf-8"))
    workspace = data.get("workspace", {})
    metadata = workspace.get("metadata", {})
    autodiscover = metadata.get("autodiscover", {})
    globs = autodiscover.get("globs", [])
    if not isinstance(globs, list):
        return []
    return [str(pattern) for pattern in globs]


def discover_members(globs: list[str]) -> list[str]:
    members: list[str] = []
    for pattern in globs:
        for path in sorted(ROOT.glob(pattern)):
            if not path.is_dir():
                continue
            if (path / "Cargo.toml").is_file():
                members.append(path.relative_to(ROOT).as_posix())
    return members


def unique_preserving_order(items: list[str]) -> list[str]:
    seen: set[str] = set()
    result: list[str] = []
    for item in items:
        if item in seen:
            continue
        seen.add(item)
        result.append(item)
    return result


def format_members(members: list[str], indent: str) -> list[str]:
    if len(members) == 1:
        return [f'{indent}members = ["{members[0]}"]']
    lines = [f"{indent}members = ["]
    for member in members:
        lines.append(f'{indent}    "{member}",')
    lines.append(f"{indent}]")
    return lines


def update_manifest(members: list[str]) -> bool:
    lines = MANIFEST.read_text(encoding="utf-8").splitlines()
    start = None
    end = None
    indent = ""
    depth = 0
    for idx, line in enumerate(lines):
        stripped = line.lstrip()
        if start is None and stripped.startswith("members"):
            start = idx
            indent = line[: len(line) - len(stripped)]
            depth += line.count("[") - line.count("]")
            if depth <= 0:
                end = idx
                break
        elif start is not None:
            depth += line.count("[") - line.count("]")
            if depth <= 0:
                end = idx
                break
    if start is None or end is None:
        raise SystemExit("workspace members array not found in Cargo.toml")

    replacement = format_members(members, indent)
    if lines[start : end + 1] == replacement:
        return False

    lines[start : end + 1] = replacement
    MANIFEST.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return True


def main() -> int:
    patterns = read_patterns()
    discovered = discover_members(patterns)
    ordered = unique_preserving_order(["backend", *discovered])
    changed = update_manifest(ordered)
    if changed:
        print("Updated workspace members:", ", ".join(ordered))
    return 0


if __name__ == "__main__":
    sys.exit(main())
