#!/usr/bin/env python3
"""Pre-flight validation for infrastructure test dependencies.

This script verifies that required command-line tools for Terraform and policy
integration tests are available before test execution. Run it as a CI step or
locally prior to invoking the infrastructure-oriented Go test suites to avoid
silent skips caused by missing binaries.
"""

from __future__ import annotations

import shutil
import sys
from dataclasses import dataclass
from typing import Iterable, List


@dataclass(frozen=True)
class Dependency:
    """Represents a required binary for the infrastructure test suites."""

    name: str
    description: str


REQUIRED_DEPENDENCIES: tuple[Dependency, ...] = (
    Dependency("tofu", "OpenTofu CLI for Terraform plan execution"),
    Dependency("conftest", "Policy testing via Open Policy Agent"),
)


def collect_missing(dependencies: Iterable[Dependency]) -> List[Dependency]:
    """Return the subset of *dependencies* that are not discoverable on PATH."""

    missing: List[Dependency] = []
    for dependency in dependencies:
        if shutil.which(dependency.name) is None:
            missing.append(dependency)
    return missing


def format_missing_message(missing: Iterable[Dependency]) -> str:
    """Create a human-readable report for *missing* dependencies."""

    lines = [
        "Required test dependencies were not found on PATH.",
        "Install the following tools before running the infrastructure tests:",
        "",
    ]
    for dependency in missing:
        lines.append(f"  - {dependency.name}: {dependency.description}")
    return "\n".join(lines)


def main() -> int:
    if len(sys.argv) > 1 and sys.argv[1] in {"-h", "--help"}:
        sys.stdout.write(
            "Run this script before executing the Go-based infrastructure tests "
            "to verify that the supporting command-line tools are installed.\n"
        )
        sys.stdout.write(
            "Install missing tools via your package manager or refer to "
            "docs/infrastructure-test-dependencies.md for guidance.\n"
        )
        return 0

    missing = collect_missing(REQUIRED_DEPENDENCIES)
    if missing:
        sys.stderr.write(format_missing_message(missing) + "\n")
        return 1

    sys.stdout.write("All required test dependencies are present.\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
