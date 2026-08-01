"""Command-level contracts for repository tooling targets."""

from __future__ import annotations

import os
import subprocess
from pathlib import Path
from shutil import which

import pytest

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]

FAKE_GIT = """#!/bin/sh
if [ "${EMPTY_GIT_OUTPUT:-0}" = 1 ]; then
    exit 0
fi
if [ "${FAIL_GIT_CACHED:-0}" = 1 ] && [ "$2" = "--cached" ]; then
    exit 42
fi
if [ "$1" = "ls-files" ]; then
    printf 'docs/zeta.md\\000docs/A.md\\000'
elif [ "$2" = "--cached" ]; then
    printf 'docs/semi;colon & brackets[1].md\\000docs/zeta.md\\000'
elif [ "$5" = "origin/main...HEAD" ]; then
    printf 'docs/a space.md\\000docs/zeta.md\\000'
else
    printf 'docs/A.md\\000docs/semi;colon & brackets[1].md\\000'
fi
"""

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
    _write_executable(fake_bin / "git", FAKE_GIT)
    _write_executable(fake_bin / "tool", FAKE_TOOL)
    for tool_name in ("bun", "pnpm", "uv"):
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


def test_nixie_discovers_all_markdown_sources_and_sets_tempdirs(
    fake_tool_environment: tuple[dict[str, str], Path],
) -> None:
    """Nixie receives every pending Markdown path and repository-local dirs."""
    env, log_path = fake_tool_environment

    completed = _run_make("nixie", env)

    assert completed.returncode == 0, completed.stderr
    invocations = _read_invocations(log_path)
    repository_tmp = str(REPOSITORY_ROOT / ".tmp")
    uv_cache = str(REPOSITORY_ROOT / ".uv-cache")
    uv_tools = str(REPOSITORY_ROOT / ".uv-tools")
    expected_bun_install = (
        "bun",
        repository_tmp,
        uv_cache,
        uv_tools,
        ("install", "--frozen-lockfile"),
    )
    expected_mermaid_setup = (
        "bun",
        repository_tmp,
        uv_cache,
        uv_tools,
        ("scripts/install-mermaid-browser.mjs",),
    )
    expected_nixie_invocation = (
        "uv",
        repository_tmp,
        uv_cache,
        uv_tools,
        (
            "tool",
            "run",
            "--python",
            "3.14",
            "--from",
            "nixie-cli@1.1.0",
            "nixie",
            "--no-sandbox",
            "--max-concurrency",
            "1",
            "--",
            "docs/A.md",
            "docs/a space.md",
            "docs/semi;colon & brackets[1].md",
            "docs/zeta.md",
        ),
    )
    assert invocations == [
        expected_bun_install,
        expected_mermaid_setup,
        expected_nixie_invocation,
    ], "Nixie must receive one bytewise ordered, duplicate-free worklist"


def test_nixie_stops_before_validation_when_discovery_fails(
    fake_tool_environment: tuple[dict[str, str], Path],
) -> None:
    """A Git discovery failure prevents Nixie from running."""
    env, log_path = fake_tool_environment
    env["FAIL_GIT_CACHED"] = "1"

    completed = _run_make("nixie", env)

    assert completed.returncode != 0
    assert not any(invocation[0] == "uv" for invocation in _read_invocations(log_path))
    assert not list((REPOSITORY_ROOT / ".tmp").glob("nixie-*")), (
        "temporary Nixie worklists must be cleaned after discovery failure"
    )


def test_nixie_uses_repository_fallback_when_worklist_is_empty(
    fake_tool_environment: tuple[dict[str, str], Path],
) -> None:
    """Nixie validates ``.`` with local directories when no paths are found."""
    env, log_path = fake_tool_environment
    env["EMPTY_GIT_OUTPUT"] = "1"

    completed = _run_make("nixie", env)

    assert completed.returncode == 0, completed.stderr
    invocations = _read_invocations(log_path)
    repository_tmp = str(REPOSITORY_ROOT / ".tmp")
    uv_cache = str(REPOSITORY_ROOT / ".uv-cache")
    uv_tools = str(REPOSITORY_ROOT / ".uv-tools")
    assert invocations[:2] == [
        ("bun", repository_tmp, uv_cache, uv_tools, ("install", "--frozen-lockfile")),
        (
            "bun",
            repository_tmp,
            uv_cache,
            uv_tools,
            ("scripts/install-mermaid-browser.mjs",),
        ),
    ], "empty discovery must retain both Nixie setup invocations"
    nixie_invocations = [
        invocation for invocation in invocations if invocation[0] == "uv"
    ]
    expected_nixie_invocation = (
        "uv",
        repository_tmp,
        uv_cache,
        uv_tools,
        (
            "tool",
            "run",
            "--python",
            "3.14",
            "--from",
            "nixie-cli@1.1.0",
            "nixie",
            "--no-sandbox",
            "--max-concurrency",
            "1",
            "--",
            ".",
        ),
    )
    assert nixie_invocations == [expected_nixie_invocation], (
        "an empty worklist must invoke Nixie exactly once with the dot fallback"
    )


def test_nixie_preserves_hostile_worktree_directory_literals(
    fake_tool_environment: tuple[dict[str, str], Path],
    tmp_path: Path,
) -> None:
    """Shell metacharacters in the worktree path remain literal environment data."""
    env, log_path = fake_tool_environment
    hostile_worktree = tmp_path / 'worktree "quoted" $(safe)\nnewline [literal]; path'
    hostile_worktree.mkdir()

    completed = _run_make("nixie", env, cwd=hostile_worktree)

    assert completed.returncode == 0, completed.stderr
    repository_tmp = hostile_worktree / ".tmp"
    uv_cache = hostile_worktree / ".uv-cache"
    uv_tools = hostile_worktree / ".uv-tools"
    assert _read_invocations(log_path) == [
        (
            "bun",
            str(repository_tmp),
            str(uv_cache),
            str(uv_tools),
            ("install", "--frozen-lockfile"),
        ),
        (
            "bun",
            str(repository_tmp),
            str(uv_cache),
            str(uv_tools),
            ("scripts/install-mermaid-browser.mjs",),
        ),
        (
            "uv",
            str(repository_tmp),
            str(uv_cache),
            str(uv_tools),
            (
                "tool",
                "run",
                "--python",
                "3.14",
                "--from",
                "nixie-cli@1.1.0",
                "nixie",
                "--no-sandbox",
                "--max-concurrency",
                "1",
                "--",
                "docs/A.md",
                "docs/a space.md",
                "docs/semi;colon & brackets[1].md",
                "docs/zeta.md",
            ),
        ),
    ], "hostile worktree characters must remain literal in exactly three invocations"


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
