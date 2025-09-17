#!/usr/bin/env python3
"""Validate review-by annotations in the Redocly ignore file."""
from __future__ import annotations

import datetime as dt
import re
import sys
from pathlib import Path

IGNORE_FILE = Path(".redocly.lint-ignore.yaml")
REVIEW_BY_PATTERN = re.compile(r"#\s*review by:\s*(\d{4}-\d{2}-\d{2})")


def main() -> None:
    try:
        lines = IGNORE_FILE.read_text(encoding="utf-8").splitlines()
    except FileNotFoundError:
        print(f"Missing {IGNORE_FILE} for review-by validation.", file=sys.stderr)
        sys.exit(1)

    today = dt.date.today()
    problems: list[str] = []

    for idx, line in enumerate(lines, start=1):
        match = REVIEW_BY_PATTERN.search(line)
        if not match:
            continue

        date_str = match.group(1)
        try:
            review_date = dt.date.fromisoformat(date_str)
        except ValueError:
            problems.append(f"line {idx}: invalid review by date '{date_str}'")
            continue

        if review_date < today:
            problems.append(
                f"line {idx}: review by {review_date.isoformat()} has already passed"
            )

    if problems:
        print(
            "Expired review-by annotations detected in .redocly.lint-ignore.yaml:",
            file=sys.stderr,
        )
        for issue in problems:
            print(f"  {issue}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
