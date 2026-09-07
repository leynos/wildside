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


#: The temporary directory the fixture hands to every Make invocation. The
#: assertions compare the tools' `TMPDIR` against this, so the contract is
#: "the Makefile passes the caller's TMPDIR through untouched" rather than
#: "whoever ran the suite happened to have no TMPDIR set". Reading the
#: ambient value made the suite pass on GitHub's runners, which set none, and
#: fail on any developer machine that sets one.
TMPDIR_SENTINEL = "tmpdir-owned-by-the-fixture"


@pytest.fixture
def fake_tool_environment(tmp_path: Path) -> tuple[dict[str, str], Path]:
    """Provide command doubles, an isolated invocation log, and a known TMPDIR."""
    fake_bin = tmp_path / "bin"
    fake_bin.mkdir()
    _write_executable(fake_bin / "tool", FAKE_TOOL)
    for tool_name in ("bun", "pnpm", "uv", "nixie", "merman-cli"):
        (fake_bin / tool_name).symlink_to(fake_bin / "tool")

    temporary = tmp_path / TMPDIR_SENTINEL
    temporary.mkdir()

    log_path = tmp_path / "tool.log"
    env = os.environ.copy()
    env["PATH"] = f"{fake_bin}{os.pathsep}{env['PATH']}"
    env["TOOL_LOG"] = str(log_path)
    env["TMPDIR"] = str(temporary)
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
        ("nixie", env["TMPDIR"], uv_cache, uv_tools, ("--renderer", "merman"))
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
        env["TMPDIR"],
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


def _assert_bounded(requirement: str) -> None:
    """Fail unless *requirement* is an exact pin or a bounded range."""
    if "==" in requirement:
        return
    assert ">=" in requirement, (
        f"{requirement!r} must be an exact pin or a bounded range"
    )
    assert "<" in requirement, (
        f"{requirement!r} needs an upper bound: an open-ended floor admits a"
        " future major release without review"
    )


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

    assert venv[:2] == ("venv", "--allow-existing"), (
        "typecheck-python must reuse an existing .venv rather than rebuild it"
    )
    assert venv[-1] == ".venv"
    assert "--python" in venv, (
        "the venv interpreter must be pinned; an unpinned `uv venv` takes"
        " whichever Python uv resolves first"
    )
    venv_python = venv[venv.index("--python") + 1]

    assert install[:5] == ("pip", "install", "--quiet", "--python", ".venv")
    requirements = install[5:]
    assert {_requirement_name(item) for item in requirements} == (
        TYPECHECK_DEPENDENCIES
    ), "ty must resolve imports against the declared dependency set"
    for requirement in requirements:
        _assert_bounded(requirement)

    assert ty[:3] == ("tool", "run", "--from")
    assert ty[3].startswith("ty==")
    assert ty[4:8] == ("ty", "check", "--python", ".venv")
    assert ty[8] == "--python-version"
    assert ty[9] == venv_python, (
        "ty must analyse the sources as the same Python version the .venv"
        " provides, or it resolves a standard library it is not checking for"
    )
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
    # `--with` requirements bypass PY_TEST_DEPS, so they need the same bounding
    # discipline: an unbounded floor silently admits a new major release.
    for index, argument in enumerate(arguments):
        if argument == "--with":
            _assert_bounded(arguments[index + 1])


#: A double that always fails, for forcing one command in a recipe to error.
FAILING_TOOL = "#!/bin/sh\nexit 3\n"

#: A double that always succeeds, so a recipe reaches the command under test.
PASSING_TOOL = "#!/bin/sh\nexit 0\n"


def test_lint_actions_fails_when_its_first_workflow_linter_fails(
    tmp_path: Path,
) -> None:
    """A yamllint failure must fail the target even though actionlint passes.

    The workflows branch of `lint-actions` runs `find | xargs uvx yamllint`
    and then `find | xargs actionlint` inside one `if`. Without a guard the
    second command's status replaces the first's, so a real yamllint failure
    would be reported as a pass. This forces exactly that ordering: the first
    linter fails, the second succeeds, and the target must still fail.
    """
    fake_bin = tmp_path / "bin"
    fake_bin.mkdir()
    _write_executable(fake_bin / "uvx", FAILING_TOOL)
    for tool_name in ("actionlint", "action-validator", "uv"):
        _write_executable(fake_bin / tool_name, PASSING_TOOL)

    env = os.environ.copy()
    env["PATH"] = f"{fake_bin}{os.pathsep}{env['PATH']}"

    completed = _run_make("lint-actions", env)

    assert completed.returncode != 0, (
        "the failing first linter did not fail lint-actions; "
        f"stdout={completed.stdout!r} stderr={completed.stderr!r}"
    )


def test_lint_actions_passes_when_every_linter_passes(tmp_path: Path) -> None:
    """The companion case, so the test above is attributable to the failure.

    Without this, a target that failed for an unrelated reason, a missing tool
    or a bad `find` invocation, would read as proof the guard works.
    """
    fake_bin = tmp_path / "bin"
    fake_bin.mkdir()
    for tool_name in ("uvx", "actionlint", "action-validator", "uv"):
        _write_executable(fake_bin / tool_name, PASSING_TOOL)

    env = os.environ.copy()
    env["PATH"] = f"{fake_bin}{os.pathsep}{env['PATH']}"

    completed = _run_make("lint-actions", env)

    assert completed.returncode == 0, (
        f"stdout={completed.stdout!r} stderr={completed.stderr!r}"
    )


#: Every linter invocation inside `lint-actions`. Each must carry its own
#: `|| exit 1`: two shapes here lose a status without one, an earlier loop
#: iteration and the first of two commands in an `if` branch, and both are
#: invisible in the output. `.SHELLFLAGS`'s `-e` also covers them, which is
#: why this assertion is static: a behavioural test cannot tell the guard and
#: the flag apart while both are present.
GUARDED_LINTERS = ("yamllint", "action-validator", "actionlint")


@pytest.mark.parametrize("linter", GUARDED_LINTERS)
def test_every_lint_actions_linter_carries_its_own_exit_guard(linter: str) -> None:
    """Each linter invocation must fail the recipe on its own."""
    makefile = (REPOSITORY_ROOT / "Makefile").read_text(encoding="utf8")
    body = makefile[makefile.index("define LINT_ACTIONS_CMD") :]
    body = body[: body.index("\nendef")]

    invocations = [
        line
        for line in body.splitlines()
        if linter in line
        # `ensure_tool` only checks the binary is present, and a comment
        # merely mentions it; neither is an invocation that can fail a lint.
        and "ensure_tool" not in line
        and not line.lstrip().startswith("#")
    ]
    assert invocations, f"lint-actions must invoke {linter}"
    unguarded = [line for line in invocations if "|| exit 1" not in line]
    assert unguarded == [], (
        f"every {linter} invocation must carry '|| exit 1'; without it a "
        f"failure is replaced by a later command's status: {unguarded}"
    )
