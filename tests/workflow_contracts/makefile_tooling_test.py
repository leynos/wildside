"""Command-level contracts for repository tooling targets."""

from __future__ import annotations

import os
import re
import subprocess  # noqa: S404 - tests deliberately exercise Make via subprocess.
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
        invocations.append((
            tool.decode(),
            tmpdir,
            uv_cache_dir,
            uv_tool_dir,
            arguments,
        ))
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


# A requirement's leading name, before any `==` pin or `>=` floor.
_REQUIREMENT_NAME = re.compile(r"[A-Za-z0-9._-]+")

# Gated separately by spelling-helper-test against its own pinned Ruff, so the
# repository-wide Python gates must leave these sources alone.
SPELLING_HELPER_SOURCES = (
    "scripts/typos_rollout_check.py",
    "scripts/tests/test_typos_rollout_check.py",
)

TYPECHECK_DEPENDENCIES = frozenset({
    "pytest",
    "pytest-mock",
    "hypothesis",
    "pyyaml",
    "cyclopts",
    "plumbum",
    "cryptography",
    "tomli",
})


def _tool_arguments(log_path: Path, tool: str) -> list[tuple[str, ...]]:
    """Return the argument tuple of each logged invocation of *tool*."""
    return [
        invocation[-1]
        for invocation in _read_invocations(log_path)
        if invocation[0] == tool
    ]


def _requirement_name(requirement: str) -> str:
    """Return the distribution name from a requirement specifier."""
    match = _REQUIREMENT_NAME.match(requirement)
    assert match is not None, f"unparsable requirement: {requirement!r}"
    return match.group()


def test_check_fmt_python_verifies_formatting_without_rewriting(
    fake_tool_environment: tuple[dict[str, str], Path],
) -> None:
    """The format gate checks Ruff formatting rather than applying it."""
    env, log_path = fake_tool_environment

    completed = _run_make("check-fmt-python", env)

    assert completed.returncode == 0, completed.stderr
    (arguments,) = _tool_arguments(log_path, "uv")
    assert arguments[:3] == ("tool", "run", "--from")
    assert arguments[3].startswith("ruff=="), "the format gate must run a pinned Ruff"
    assert arguments[4:] == ("ruff", "format", "--check"), (
        "check-fmt-python must verify formatting without writing files"
    )


def test_lint_python_runs_ruff_interrogate_and_pylint(
    fake_tool_environment: tuple[dict[str, str], Path],
) -> None:
    """The Python lint gate runs all three configured tiers, in order."""
    env, log_path = fake_tool_environment

    completed = _run_make("lint-python", env)

    assert completed.returncode == 0, completed.stderr
    ruff, interrogate, pylint = _tool_arguments(log_path, "uv")

    assert ruff[:3] == ("tool", "run", "--from")
    assert ruff[3].startswith("ruff==")
    assert ruff[4:] == ("ruff", "check")

    assert interrogate[:3] == ("tool", "run", "--from")
    assert interrogate[3].startswith("interrogate==")
    assert interrogate[4:] == (
        "interrogate",
        "--fail-under",
        "100",
        "scripts",
    ), "interrogate must keep demanding total docstring coverage of scripts"

    assert pylint[:3] == ("tool", "run", "--python")
    assert pylint[4] == "--from"
    assert pylint[5].startswith(
        "git+https://github.com/leynos/pylint-pypy-shim.git@"
    ), "Pylint must run through the pinned PyPy shim"
    assert pylint[6:] == ("pylint-pypy", "scripts", "tests"), (
        "Pylint must cover both configured target trees"
    )


def test_typecheck_python_materializes_a_venv_before_running_ty(
    fake_tool_environment: tuple[dict[str, str], Path],
) -> None:
    """The gate materializes an environment before ty resolves imports."""
    env, log_path = fake_tool_environment

    completed = _run_make("typecheck-python", env)

    assert completed.returncode == 0, completed.stderr
    venv, install, ty = _tool_arguments(log_path, "uv")

    assert venv == ("venv", "--allow-existing", ".venv"), (
        "typecheck-python must reuse an existing .venv rather than rebuild it"
    )

    assert install[:5] == ("pip", "install", "--quiet", "--python", ".venv")
    requirements = install[5:]
    assert {_requirement_name(item) for item in requirements} == (
        TYPECHECK_DEPENDENCIES
    ), "ty must resolve imports against the declared dependency set"
    assert all(("==" in item or ">=" in item) for item in requirements), (
        "every typecheck dependency must carry a version constraint"
    )

    assert ty[:3] == ("tool", "run", "--from")
    assert ty[3].startswith("ty==")
    assert ty[4:10] == (
        "ty",
        "check",
        "--python",
        ".venv",
        "--python-version",
        "3.13",
    ), "ty must check the materialized environment at the pinned version"
    sources = ty[10:]
    assert sources, "ty must receive the configured Python sources"
    assert all(source.endswith(".py") for source in sources)
    assert "scripts/local_k8s.py" in sources, (
        "the preview CLI must stay within the typecheck surface"
    )
    assert not set(sources) & set(SPELLING_HELPER_SOURCES), (
        "the separately gated spelling helper must stay excluded"
    )


# The root pyproject.toml carries tooling configuration only, so every uv
# invocation opts out of project discovery to keep its resolution isolated.
NO_PROJECT_TARGETS = (
    ("local-k8s-up", ("scripts/local_k8s.py", "up")),
    ("local-k8s-down", ("scripts/local_k8s.py", "down")),
    ("local-k8s-status", ("scripts/local_k8s.py", "status")),
    ("local-k8s-logs", ("scripts/local_k8s.py", "logs")),
    ("test-workflow-contracts", ("tests/workflow_contracts",)),
    ("test-scripts", ("scripts/local_k8s/unittests",)),
)


@pytest.mark.parametrize(("target", "expected_arguments"), NO_PROJECT_TARGETS)
def test_targets_run_uv_without_project_discovery(
    target: str,
    expected_arguments: tuple[str, ...],
    fake_tool_environment: tuple[dict[str, str], Path],
) -> None:
    """Helper and test targets resolve without the tooling-only project."""
    env, log_path = fake_tool_environment

    completed = _run_make(target, env)

    assert completed.returncode == 0, completed.stderr
    (arguments,) = _tool_arguments(log_path, "uv")
    assert arguments[:2] == ("run", "--no-project"), (
        f"{target} must run uv with --no-project so the root pyproject.toml"
        " cannot alter its resolution"
    )
    for expected in expected_arguments:
        assert expected in arguments, f"{target} must pass {expected!r} to uv run"
