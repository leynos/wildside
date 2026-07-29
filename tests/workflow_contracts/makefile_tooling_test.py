"""Command-level contracts for repository tooling targets."""

from __future__ import annotations

import os
import subprocess
from pathlib import Path
from shutil import which

import pytest

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]

FAKE_TOOL = """#!/bin/sh
printf '%s\\0%s\\0%s\\0%s\\0%s\\0' \\
    "$(basename "$0")" \\
    "${TMPDIR:-}" \\
    "${UV_CACHE_DIR:-}" \\
    "${UV_TOOL_DIR:-}" \\
    "$#" >> "$TOOL_LOG"
for argument in "$@"; do
    printf '%s\\0' "$argument" >> "$TOOL_LOG"
done
"""

type ToolInvocation = tuple[str, str, str, str, tuple[str, ...]]


def _write_executable(path: Path, source: str) -> None:
    """Write an executable command double."""
    path.write_text(source, encoding="utf8")
    path.chmod(0o755)


@pytest.fixture
def fake_tool_environment(tmp_path: Path) -> tuple[dict[str, str], Path]:
    """Provide command doubles and an isolated invocation log."""
    fake_bin = tmp_path / "bin"
    fake_bin.mkdir()
    _write_executable(fake_bin / "tool", FAKE_TOOL)
    for tool_name in ("bun", "pnpm", "uv", "nixie", "merman-cli"):
        (fake_bin / tool_name).symlink_to(fake_bin / "tool")

    log_path = tmp_path / "tool.log"
    env = os.environ.copy()
    env["PATH"] = f"{fake_bin}{os.pathsep}{env['PATH']}"
    env["TOOL_LOG"] = str(log_path)
    return env, log_path


def _run_make(
    target: str,
    env: dict[str, str],
    *,
    cwd: Path = REPOSITORY_ROOT,
) -> subprocess.CompletedProcess[str]:
    """Run a Makefile tooling target with command doubles on ``PATH``."""
    make = which("make")
    assert make is not None, "make must be available for workflow contract tests"
    fake_uv = Path(env["PATH"].split(os.pathsep, maxsplit=1)[0]) / "uv"
    return subprocess.run(  # noqa: S603 - the resolved make executable is trusted.
        [
            make,
            "--no-print-directory",
            "-f",
            str(REPOSITORY_ROOT / "Makefile"),
            f"PATH={env['PATH']}",
            f"UV={fake_uv}",
            target,
        ],
        cwd=cwd,
        env=env,
        text=True,
        capture_output=True,
        check=False,
        timeout=30,
    )


def _read_invocations(log_path: Path) -> list[ToolInvocation]:
    """Return logged command invocations, or an empty list when none ran."""
    if not log_path.exists():
        return []
    fields = iter(log_path.read_bytes().removesuffix(b"\0").split(b"\0"))
    invocations = []
    while tool := next(fields, None):
        tmpdir = next(fields).decode()
        uv_cache_dir = next(fields).decode()
        uv_tool_dir = next(fields).decode()
        argument_count = int(next(fields))
        arguments = tuple(next(fields).decode() for _ in range(argument_count))
        invocations.append(
            (tool.decode(), tmpdir, uv_cache_dir, uv_tool_dir, arguments)
        )
    return invocations


def test_nixie_invokes_the_installed_merman_renderer(
    fake_tool_environment: tuple[dict[str, str], Path],
) -> None:
    """Nixie invokes its installed Merman renderer without setup commands."""
    env, log_path = fake_tool_environment

    completed = _run_make("nixie", env)

    assert completed.returncode == 0, completed.stderr
    uv_cache = str(REPOSITORY_ROOT / ".uv-cache")
    uv_tools = str(REPOSITORY_ROOT / ".uv-tools")
    assert _read_invocations(log_path) == [
        ("nixie", "", uv_cache, uv_tools, ("--renderer", "merman"))
    ]


def test_nixie_stops_before_validation_without_merman(
    fake_tool_environment: tuple[dict[str, str], Path],
) -> None:
    """A missing installed renderer prevents Nixie from running."""
    env, log_path = fake_tool_environment
    fake_bin = Path(env["PATH"].split(os.pathsep, maxsplit=1)[0])
    (fake_bin / "merman-cli").unlink()
    env["PATH"] = str(fake_bin)

    completed = _run_make("nixie", env)

    assert completed.returncode != 0
    assert _read_invocations(log_path) == []


def test_lint_asyncapi_uses_pnpm_cli_runner(
    fake_tool_environment: tuple[dict[str, str], Path],
) -> None:
    """The AsyncAPI target selects pnpm and passes the validation contract."""
    env, log_path = fake_tool_environment

    completed = _run_make("lint-asyncapi", env)

    assert completed.returncode == 0, completed.stderr
    expected_invocation = (
        "pnpm",
        "",
        str(REPOSITORY_ROOT / ".uv-cache"),
        str(REPOSITORY_ROOT / ".uv-tools"),
        (
            "dlx",
            "@asyncapi/cli@3.4.2",
            "validate",
            "spec/asyncapi.yaml",
            "--fail-severity=info",
        ),
    )
    assert _read_invocations(log_path) == [expected_invocation]
