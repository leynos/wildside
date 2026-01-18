"""Pytest configuration for scripts tests."""

from __future__ import annotations

import sys
from pathlib import Path


def pytest_configure() -> None:
    """Ensure the repository root is available on sys.path before collection."""
    repo_root = Path(__file__).resolve().parents[2]
    if str(repo_root) not in sys.path:
        sys.path.insert(0, str(repo_root))
