from __future__ import annotations

import sys
from pathlib import Path


def pytest_configure() -> None:
    project_root = Path(__file__).resolve().parents[2]
    if str(project_root) not in sys.path:
        sys.path.insert(0, str(project_root))
    vendor_dir = Path(__file__).resolve().parent / "_vendor"
    if str(vendor_dir) not in sys.path:
        sys.path.insert(0, str(vendor_dir))
