"""Tests and helpers for local-preview fake command logs."""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path

import pytest


@dataclass(frozen=True)
class _LogEntriesCase:
    source: str
    expected: list[list[object]] | None
    expected_error: type[Exception] | None
    message: str | None


def _load_log_entries(log_path: Path) -> list[list[object]]:
    """Load validated fake-tool command records from a JSON-lines log."""
    entries: list[list[object]] = []
    for line_number, line in enumerate(
        log_path.read_text(encoding="utf8").splitlines(), start=1
    ):
        decoded = json.loads(line)
        is_command_record = (
            isinstance(decoded, list)
            and len(decoded) == 3
            and isinstance(decoded[0], str)
            and isinstance(decoded[1], list)
            and all(isinstance(argument, str) for argument in decoded[1])
            and isinstance(decoded[2], bool)
        )
        assert is_command_record, (
            f"fake tool log line {line_number} must decode to "
            f"[str, list[str], bool], got {type(decoded).__name__}: {decoded!r}"
        )
        entries.append(decoded)
    return entries


@pytest.mark.parametrize(
    "case",
    [
        pytest.param(
            _LogEntriesCase(
                source='["docker", ["build"], false]\n["helm", ["status"], true]\n',
                expected=[["docker", ["build"], False], ["helm", ["status"], True]],
                expected_error=None,
                message=None,
            ),
            id="multiple-lists",
        ),
        pytest.param(
            _LogEntriesCase(
                source="[]\n",
                expected=None,
                expected_error=AssertionError,
                message="line 1.*list",
            ),
            id="wrong-length",
        ),
        pytest.param(
            _LogEntriesCase(
                source='[1, ["build"], false]\n',
                expected=None,
                expected_error=AssertionError,
                message="line 1.*list",
            ),
            id="non-string-tool",
        ),
        pytest.param(
            _LogEntriesCase(
                source='["docker", "build", false]\n',
                expected=None,
                expected_error=AssertionError,
                message="line 1.*list",
            ),
            id="non-list-arguments",
        ),
        pytest.param(
            _LogEntriesCase(
                source='["docker", [1], false]\n',
                expected=None,
                expected_error=AssertionError,
                message="line 1.*list",
            ),
            id="non-string-argument",
        ),
        pytest.param(
            _LogEntriesCase(
                source='["docker", ["build"], 0]\n',
                expected=None,
                expected_error=AssertionError,
                message="line 1.*list",
            ),
            id="non-boolean-stdin",
        ),
        pytest.param(
            _LogEntriesCase(
                source='{"tool": "docker"}\n',
                expected=None,
                expected_error=AssertionError,
                message="line 1.*dict",
            ),
            id="object",
        ),
        pytest.param(
            _LogEntriesCase(
                source="42\n",
                expected=None,
                expected_error=AssertionError,
                message="line 1.*int",
            ),
            id="scalar",
        ),
        pytest.param(
            _LogEntriesCase(
                source="{invalid\n",
                expected=None,
                expected_error=json.JSONDecodeError,
                message=None,
            ),
            id="invalid-json",
        ),
    ],
)
def test_load_log_entries_validates_each_json_line(
    tmp_path: Path,
    case: _LogEntriesCase,
) -> None:
    """Return list entries and reject other decoded JSON shapes."""
    log_path = tmp_path / "commands.jsonl"
    log_path.write_text(case.source, encoding="utf8")

    if case.expected_error is None:
        assert _load_log_entries(log_path) == case.expected, (
            "decoded fake-tool records must match the complete command log"
        )
    else:
        with pytest.raises(case.expected_error, match=case.message):
            _load_log_entries(log_path)
