#!/usr/bin/env python3
"""Validate review-by annotations in the Redocly ignore file."""
from __future__ import annotations

import datetime as dt
import re
import sys
from pathlib import Path

IGNORE_FILE = Path(".redocly.lint-ignore.yaml")
REVIEW_BY_PATTERN = re.compile(r"#\s*review by:\s*(\d{4}-\d{2}-\d{2})")


def load_ignore_file_lines() -> list[str]:
    """Read the ignore file and return its lines.

    Examples:
        >>> lines = load_ignore_file_lines()  # doctest: +SKIP
        >>> isinstance(lines, list)
        True
    """

    try:
        return IGNORE_FILE.read_text(encoding="utf-8").splitlines()
    except FileNotFoundError:
        print(f"Missing {IGNORE_FILE} for review-by validation.", file=sys.stderr)
        sys.exit(1)


def extract_review_date(line: str) -> str | None:
    """Return the review-by date contained within a line, if present.

    Examples:
        >>> extract_review_date('# review by: 2025-01-01')
        '2025-01-01'
        >>> extract_review_date('no annotation here') is None
        True
    """

    match = REVIEW_BY_PATTERN.search(line)
    if not match:
        return None
    return match.group(1)


def parse_review_date(date_str: str, line_num: int) -> dt.date | None:
    """Parse a review-by date, returning ``None`` when invalid.

    Examples:
        >>> parse_review_date('2025-01-01', 3)
        datetime.date(2025, 1, 1)
        >>> parse_review_date('invalid-date', 4) is None
        True
    """

    try:
        return dt.date.fromisoformat(date_str)
    except ValueError:
        return None


def validate_review_line(line: str, line_num: int, today: dt.date) -> str | None:
    """Validate a review-by annotation and report any problems.

    Examples:
        >>> today = dt.date(2025, 1, 1)
        >>> validate_review_line('# review by: 2024-12-31', 8, today)
        "line 8: review by 2024-12-31 has already passed"
        >>> validate_review_line('# review by: 2025-01-02', 9, today) is None
        True
    """

    date_str = extract_review_date(line)
    if date_str is None:
        return None

    review_date = parse_review_date(date_str, line_num)
    if review_date is None:
        return f"line {line_num}: invalid review by date '{date_str}'"

    if review_date < today:
        return f"line {line_num}: review by {review_date.isoformat()} has already passed"

    return None


def collect_problems(lines: list[str], today: dt.date) -> list[str]:
    """Return all review-by issues detected in *lines*.

    Examples:
        >>> today = dt.date(2025, 1, 1)
        >>> collect_problems(['# review by: 2024-12-31'], today)
        ["line 1: review by 2024-12-31 has already passed"]
    """

    return [
        issue
        for idx, line in enumerate(lines, start=1)
        if (issue := validate_review_line(line, idx, today))
    ]


def report_problems(problems: list[str]) -> None:
    """Emit diagnostics for the collected *problems*."""

    print(
        "Expired review-by annotations detected in .redocly.lint-ignore.yaml:",
        file=sys.stderr,
    )
    for issue in problems:
        print(f"  {issue}", file=sys.stderr)


def main() -> None:
    problems = collect_problems(load_ignore_file_lines(), dt.date.today())
    if not problems:
        return

    report_problems(problems)
    sys.exit(1)


if __name__ == "__main__":
    main()
