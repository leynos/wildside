"""Pytest configuration for scripts tests."""

from __future__ import annotations

import sys
from pathlib import Path

import pytest


@pytest.fixture(autouse=True, scope="session")
def _setup_path() -> None:
    """Ensure the repository root is available on sys.path."""
    repo_root = Path(__file__).resolve().parents[2]
    if str(repo_root) not in sys.path:
        sys.path.insert(0, str(repo_root))
