"""Tests and helpers for local-preview fake command logs."""

from __future__ import annotations

import json
from pathlib import Path

import pytest


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
    ("source", "expected", "expected_error", "message"),
    [
        pytest.param(
            '["docker", ["build"], false]\n["helm", ["status"], true]\n',
            [["docker", ["build"], False], ["helm", ["status"], True]],
            None,
            None,
            id="multiple-lists",
        ),
        pytest.param("[]\n", None, AssertionError, "line 1.*list", id="wrong-length"),
        pytest.param(
            '[1, ["build"], false]\n',
            None,
            AssertionError,
            "line 1.*list",
            id="non-string-tool",
        ),
        pytest.param(
            '["docker", "build", false]\n',
            None,
            AssertionError,
            "line 1.*list",
            id="non-list-arguments",
        ),
        pytest.param(
            '["docker", [1], false]\n',
            None,
            AssertionError,
            "line 1.*list",
            id="non-string-argument",
        ),
        pytest.param(
            '["docker", ["build"], 0]\n',
            None,
            AssertionError,
            "line 1.*list",
            id="non-boolean-stdin",
        ),
        pytest.param(
            '{"tool": "docker"}\n', None, AssertionError, "line 1.*dict", id="object"
        ),
        pytest.param("42\n", None, AssertionError, "line 1.*int", id="scalar"),
        pytest.param("{invalid\n", None, json.JSONDecodeError, None, id="invalid-json"),
    ],
)
def test_load_log_entries_validates_each_json_line(
    tmp_path: Path,
    source: str,
    expected: list[list[object]] | None,
    expected_error: type[Exception] | None,
    message: str | None,
) -> None:
    """Return list entries and reject other decoded JSON shapes."""
    log_path = tmp_path / "commands.jsonl"
    log_path.write_text(source, encoding="utf8")

    if expected_error is None:
        assert _load_log_entries(log_path) == expected, (
            "decoded fake-tool records must match the complete command log"
        )
    else:
        with pytest.raises(expected_error, match=message):
            _load_log_entries(log_path)
