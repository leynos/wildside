#!/usr/bin/env python3
"""Pre-flight validation for infrastructure test dependencies.

This script verifies that required command-line tools for Terraform and policy
integration tests are available before test execution. Run it as a CI step or
locally prior to invoking the infrastructure-oriented Go test suites to avoid
silent skips caused by missing binaries.
"""

from __future__ import annotations

import argparse
import re
import shutil
import subprocess
import sys
from dataclasses import dataclass
from typing import Iterable, Sequence


@dataclass(frozen=True)
class Dependency:
    """Represents a required binary for the infrastructure test suites."""

    name: str
    description: str
    minimum_version: str | None = None
    version_args: tuple[str, ...] = ("--version",)


@dataclass(frozen=True)
class VersionProbeResult:
    """Outcome of invoking a dependency's version command."""

    parsed_version: str | None
    raw_output: str | None


REQUIRED_DEPENDENCIES: tuple[Dependency, ...] = (
    Dependency(
        "tofu",
        "OpenTofu CLI for Terraform plan execution",
        minimum_version="1.7.0",
        version_args=("version",),
    ),
    Dependency(
        "conftest",
        "Policy testing via Open Policy Agent",
        minimum_version="0.45.0",
    ),
)

VERSION_PATTERN = re.compile(r"(\d+(?:\.\d+)+)")


def collect_missing(dependencies: Iterable[Dependency]) -> list[Dependency]:
    """Return the subset of *dependencies* that are not discoverable on PATH."""

    return [
        dependency
        for dependency in dependencies
        if shutil.which(dependency.name) is None
    ]


def to_int_tuple(version: str) -> tuple[int, ...]:
    """Convert a dotted version string into a tuple of integers."""

    return tuple(int(part) for part in version.split("."))


def is_version_sufficient(installed: str, minimum: str) -> bool:
    """Return ``True`` if *installed* satisfies the *minimum* requirement."""

    installed_parts = to_int_tuple(installed)
    minimum_parts = to_int_tuple(minimum)
    max_length = max(len(installed_parts), len(minimum_parts))
    installed_normalised = installed_parts + (0,) * (max_length - len(installed_parts))
    minimum_normalised = minimum_parts + (0,) * (max_length - len(minimum_parts))
    return installed_normalised >= minimum_normalised


def parse_version_from_output(output: str) -> str | None:
    """Extract a dotted version string from *output*, if present."""

    match = VERSION_PATTERN.search(output)
    if match is None:
        return None
    return match.group(1)


def probe_version(dependency: Dependency) -> VersionProbeResult:
    """Run the version command for *dependency* and parse the response."""

    try:
        process = subprocess.run(
            (dependency.name, *dependency.version_args),
            capture_output=True,
            text=True,
            check=True,
        )
    except (OSError, subprocess.CalledProcessError):
        return VersionProbeResult(parsed_version=None, raw_output=None)

    output = process.stdout.strip() or process.stderr.strip() or None
    if output is None:
        return VersionProbeResult(parsed_version=None, raw_output=None)

    parsed_version = parse_version_from_output(output)
    return VersionProbeResult(parsed_version=parsed_version, raw_output=output)


def validate_dependencies(
    dependencies: Iterable[Dependency],
) -> tuple[list[Dependency], list[tuple[Dependency, str | None]]]:
    """Check for missing dependencies and incompatible versions."""

    missing = collect_missing(dependencies)
    incompatible: list[tuple[Dependency, str | None]] = []

    for dependency in dependencies:
        if dependency in missing:
            continue
        if dependency.minimum_version is None:
            continue

        probe = probe_version(dependency)
        if probe.parsed_version is None:
            incompatible.append((dependency, probe.raw_output))
            continue

        if not is_version_sufficient(probe.parsed_version, dependency.minimum_version):
            incompatible.append((dependency, probe.raw_output or probe.parsed_version))

    return missing, incompatible


def format_failure_message(
    missing: Iterable[Dependency],
    incompatible: Iterable[tuple[Dependency, str | None]],
) -> str:
    """Create a human-readable report for dependency validation failures."""

    missing_list = list(missing)
    incompatible_list = list(incompatible)

    lines = ["Required test dependencies failed validation."]

    if missing_list:
        lines.append("")
        lines.append(
            "Install the following tools before running the infrastructure tests:"
        )
        lines.extend(
            f"  - {dependency.name}: {dependency.description}"
            for dependency in missing_list
        )

    if incompatible_list:
        if missing_list:
            lines.append("")
        lines.append("Update the following tools to satisfy minimum supported versions:")
        for dependency, detected in incompatible_list:
            required = dependency.minimum_version or "unspecified"
            observed = (detected or "unknown").splitlines()[0]
            lines.append(
                f"  - {dependency.name}: found {observed}, require >= {required}"
            )

    lines.extend(
        [
            "",
            "Refer to docs/infrastructure-test-dependencies.md for installation guidance.",
        ]
    )
    return "\n".join(lines)


def format_markdown_table(dependencies: Sequence[Dependency]) -> str:
    """Render a Markdown table describing *dependencies*."""

    lines = [
        "| Tool | Minimum version | Purpose |",
        "| ---- | ---------------- | ------- |",
    ]
    lines.extend(
        f"| `{dependency.name}` | {dependency.minimum_version or 'n/a'} | {dependency.description} |"
        for dependency in dependencies
    )
    return "\n".join(lines)


def build_parser() -> argparse.ArgumentParser:
    """Construct the command-line interface for the script."""

    parser = argparse.ArgumentParser(
        description=(
            "Validate command-line dependencies for the infrastructure test suites "
            "or emit documentation describing the required tooling."
        )
    )
    parser.add_argument(
        "--emit-markdown",
        action="store_true",
        help="Print a Markdown table of required dependencies and exit.",
    )
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)

    if args.emit_markdown:
        sys.stdout.write(format_markdown_table(REQUIRED_DEPENDENCIES) + "\n")
        return 0

    missing, incompatible = validate_dependencies(REQUIRED_DEPENDENCIES)
    if missing or incompatible:
        sys.stderr.write(format_failure_message(missing, incompatible) + "\n")
        return 1

    sys.stdout.write("All required test dependencies are present and compatible.\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
