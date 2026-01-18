"""Pytest configuration for scripts tests.

Notes
-----
Ensures the repository root is on ``sys.path`` so script modules import
correctly during collection.

Examples
--------
>>> # Pytest invokes pytest_configure automatically during collection.
"""

from __future__ import annotations

import sys
from pathlib import Path


def pytest_configure() -> None:
    """Configure sys.path for script test collection.

    Notes
    -----
    Adds the repository root to ``sys.path`` so tests can import scripts.
    """
    repo_root = Path(__file__).resolve().parents[2]
    if str(repo_root) not in sys.path:
        sys.path.insert(0, str(repo_root))
