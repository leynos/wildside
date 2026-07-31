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
printf '%s|%s|%s|%s|%s\\n' \\
    "$(basename "$0")" \\
    "${TMPDIR:-}" \\
    "${UV_CACHE_DIR:-}" \\
    "${UV_TOOL_DIR:-}" \\
    "$*" >> "$TOOL_LOG"
"""


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


def _read_invocations(log_path: Path) -> list[str]:
    """Return logged command invocations, or an empty list when none ran."""
    if not log_path.exists():
        return []
    return log_path.read_text(encoding="utf8").splitlines()


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
    assert invocations[0] == (
        f"bun|{repository_tmp}|{uv_cache}|{uv_tools}|install --frozen-lockfile"
    )
    assert invocations[1] == (
        f"bun|{repository_tmp}|{uv_cache}|{uv_tools}|"
        "scripts/install-mermaid-browser.mjs"
    )
    expected_nixie_invocation = (
        f"uv|{repository_tmp}|{uv_cache}|{uv_tools}|"
        "tool run --python 3.14 --from nixie-cli@1.1.0 nixie "
        "--no-sandbox --max-concurrency 1 -- docs/A.md docs/a space.md "
        "docs/semi;colon & brackets[1].md docs/zeta.md"
    )
    assert invocations == [invocations[0], invocations[1], expected_nixie_invocation]


def test_nixie_stops_before_validation_when_discovery_fails(
    fake_tool_environment: tuple[dict[str, str], Path],
) -> None:
    """A Git discovery failure prevents Nixie from running."""
    env, log_path = fake_tool_environment
    env["FAIL_GIT_CACHED"] = "1"

    completed = _run_make("nixie", env)

    assert completed.returncode != 0
    assert not any(
        invocation.startswith("uv|") for invocation in _read_invocations(log_path)
    )
    assert not list((REPOSITORY_ROOT / ".tmp").glob("nixie-*"))


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
        f"bun|{repository_tmp}|{uv_cache}|{uv_tools}|install --frozen-lockfile",
        f"bun|{repository_tmp}|{uv_cache}|{uv_tools}|"
        "scripts/install-mermaid-browser.mjs",
    ]
    nixie_invocations = [
        invocation for invocation in invocations if invocation.startswith("uv|")
    ]
    expected_nixie_invocation = (
        f"uv|{repository_tmp}|{uv_cache}|{uv_tools}|"
        "tool run --python 3.14 --from nixie-cli@1.1.0 nixie "
        "--no-sandbox --max-concurrency 1 -- ."
    )
    assert nixie_invocations == [expected_nixie_invocation]


def test_nixie_preserves_hostile_worktree_directory_literals(
    fake_tool_environment: tuple[dict[str, str], Path],
    tmp_path: Path,
) -> None:
    """Shell metacharacters in the worktree path remain literal environment data."""
    env, log_path = fake_tool_environment
    hostile_worktree = tmp_path / "worktree [safe]; literal path"
    hostile_worktree.mkdir()

    completed = _run_make("nixie", env, cwd=hostile_worktree)

    assert completed.returncode == 0, completed.stderr
    repository_tmp = hostile_worktree / ".tmp"
    uv_cache = hostile_worktree / ".uv-cache"
    uv_tools = hostile_worktree / ".uv-tools"
    expected_environment = f"{repository_tmp}|{uv_cache}|{uv_tools}|"
    expected_nixie_arguments = (
        "tool run --python 3.14 --from nixie-cli@1.1.0 nixie "
        "--no-sandbox --max-concurrency 1 -- docs/A.md docs/a space.md "
        "docs/semi;colon & brackets[1].md docs/zeta.md"
    )
    assert _read_invocations(log_path) == [
        f"bun|{expected_environment}install --frozen-lockfile",
        f"bun|{expected_environment}scripts/install-mermaid-browser.mjs",
        f"uv|{expected_environment}{expected_nixie_arguments}",
    ]


def test_lint_asyncapi_uses_pnpm_cli_runner(
    fake_tool_environment: tuple[dict[str, str], Path],
) -> None:
    """The AsyncAPI target selects pnpm and passes the validation contract."""
    env, log_path = fake_tool_environment

    completed = _run_make("lint-asyncapi", env)

    assert completed.returncode == 0, completed.stderr
    expected_invocation = (
        f"pnpm||{REPOSITORY_ROOT / '.uv-cache'}|"
        f"{REPOSITORY_ROOT / '.uv-tools'}|"
        "dlx @asyncapi/cli@3.4.2 validate "
        "spec/asyncapi.yaml --fail-severity=info"
    )
    assert _read_invocations(log_path) == [expected_invocation]
