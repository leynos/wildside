"""Command-level contracts for repository tooling targets."""

from __future__ import annotations

import os
import re
from pathlib import Path

import pytest
from tooling_doubles import (
    REPOSITORY_ROOT,
    build_fake_tool_environment,
    makefile_variable,
    read_invocations,
    run_make,
    tool_arguments,
)


@pytest.fixture
def fake_tool_environment(tmp_path: Path) -> tuple[dict[str, str], Path]:
    """Provide command doubles, an isolated invocation log, and a known TMPDIR."""
    return build_fake_tool_environment(tmp_path)


def test_nixie_invokes_the_installed_merman_renderer(
    fake_tool_environment: tuple[dict[str, str], Path],
) -> None:
    """Nixie invokes its installed Merman renderer without setup commands."""
    env, log_path = fake_tool_environment

    completed = run_make("nixie", env)

    assert completed.returncode == 0, completed.stderr
    uv_cache = str(REPOSITORY_ROOT / ".uv-cache")
    uv_tools = str(REPOSITORY_ROOT / ".uv-tools")
    assert read_invocations(log_path) == [
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

    completed = run_make("nixie", env)

    assert completed.returncode != 0
    invocations = read_invocations(log_path)
    assert not invocations, f"unexpected tool invocations: {invocations!r}"


def test_lint_asyncapi_uses_pnpm_cli_runner(
    fake_tool_environment: tuple[dict[str, str], Path],
) -> None:
    """The AsyncAPI target selects pnpm and passes the validation contract."""
    env, log_path = fake_tool_environment

    completed = run_make("lint-asyncapi", env)

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
    assert read_invocations(log_path) == [expected_invocation]


# A requirement's leading name, before any `==` pin or `>=` floor.
_REQUIREMENT_NAME = re.compile(r"[A-Za-z0-9._-]+")

TYPECHECK_DEPENDENCIES = frozenset({
    "pytest",
    "pytest-mock",
    "hypothesis",
    "pyyaml",
    "cyclopts",
    "plumbum",
    "cryptography",
    "tomli",
    # `scripts/lint_actions.py` imports cuprum, the process runner the
    # scripting standards name, and its tests import cmd-mox. ty resolves
    # imports against this environment, so an omission here reads as an
    # unresolved-import error rather than a missing dependency.
    "cuprum",
    "cmd-mox",
})


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

    completed = run_make("check-fmt-python", env)

    assert completed.returncode == 0, completed.stderr
    (arguments,) = tool_arguments(log_path, "uv")
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

    completed = run_make("lint-python", env)

    assert completed.returncode == 0, completed.stderr
    ruff, interrogate, pylint = tool_arguments(log_path, "uv")

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

    assert pylint[:5] == ("tool", "run", "--managed-python", "--python", "pypy@3.12"), (
        "Pylint must run on managed PyPy pinned to its minor version"
    )
    assert pylint[5] == "--from"
    assert pylint[6].startswith("pylint=="), "Pylint must run a pinned release"
    assert pylint[7:] == ("pylint", "scripts", "tests"), (
        "Pylint must cover both configured target trees"
    )


def test_typecheck_python_materializes_a_venv_before_running_ty(
    fake_tool_environment: tuple[dict[str, str], Path],
) -> None:
    """The gate materializes an environment before ty resolves imports."""
    env, log_path = fake_tool_environment

    completed = run_make("typecheck-python", env)

    assert completed.returncode == 0, completed.stderr
    venv, install, ty = tool_arguments(log_path, "uv")

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


# The root pyproject.toml carries tooling configuration only, so every uv
# invocation opts out of project discovery to keep its resolution isolated.
NO_PROJECT_TARGETS = (
    ("local-k8s-up", ("scripts/local_k8s.py", "up")),
    ("local-k8s-down", ("scripts/local_k8s.py", "down")),
    ("local-k8s-status", ("scripts/local_k8s.py", "status")),
    ("local-k8s-logs", ("scripts/local_k8s.py", "logs")),
    ("test-workflow-contracts", ("tests/workflow_contracts", "--doctest-modules")),
    (
        "test-scripts",
        (
            "scripts/local_k8s",
            "scripts/check_redoc_ignore.py",
            "scripts/lint_actions.py",
            "scripts/sync_workspace_members.py",
            "--doctest-modules",
        ),
    ),
)


def test_lint_frontend_runs_biome_before_the_package_linter(
    fake_tool_environment: tuple[dict[str, str], Path],
) -> None:
    """The front-end lint target runs the pinned Biome check, then Oxlint."""
    env, log_path = fake_tool_environment

    completed = run_make("lint-frontend", env)

    assert completed.returncode == 0, completed.stderr
    assert [invocation[0] for invocation in read_invocations(log_path)] == [
        "biome",
        "pnpm",
    ], "lint-frontend must run Biome first and the package linter second"
    (biome,) = tool_arguments(log_path, "biome")
    assert biome == (
        "ci",
        "--formatter-enabled=true",
        "--reporter=github",
        "frontend-pwa",
        "packages",
    ), "the formatter gate must run the pinned Biome check over both workspaces"
    (oxlint,) = tool_arguments(log_path, "pnpm")
    assert oxlint == ("--filter", "frontend-pwa", "lint"), (
        "the pinned Oxlint profiles must run through the workspace script"
    )


def test_lint_frontend_uses_the_pin_when_the_path_biome_differs(
    fake_tool_environment: tuple[dict[str, str], Path],
) -> None:
    """A PATH Biome at another version is replaced by the pinned release.

    Biome's formatter output is not stable across releases, so a developer
    machine with a different Biome must not decide whether the gate passes.
    """
    env, log_path = fake_tool_environment
    env["FAKE_BIOME_VERSION"] = "0.0.0-not-the-pin"

    completed = run_make("lint-frontend", env)

    assert completed.returncode == 0, completed.stderr
    assert [invocation[0] for invocation in read_invocations(log_path)] == [
        "bun",
        "pnpm",
    ], "a mismatched PATH Biome must be replaced by the pinned release"
    (bun,) = tool_arguments(log_path, "bun")
    assert bun == (
        "x",
        f"--package=@biomejs/biome@{makefile_variable('BIOME_VERSION')}",
        "biome",
        "ci",
        "--formatter-enabled=true",
        "--reporter=github",
        "frontend-pwa",
        "packages",
    ), "the fallback must run the pinned Biome check over both workspaces"


@pytest.mark.parametrize(("target", "expected_arguments"), NO_PROJECT_TARGETS)
def test_targets_run_uv_without_project_discovery(
    target: str,
    expected_arguments: tuple[str, ...],
    fake_tool_environment: tuple[dict[str, str], Path],
) -> None:
    """Helper and test targets resolve without the tooling-only project."""
    env, log_path = fake_tool_environment

    completed = run_make(target, env)

    assert completed.returncode == 0, completed.stderr
    (arguments,) = tool_arguments(log_path, "uv")
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
