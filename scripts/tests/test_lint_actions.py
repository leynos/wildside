"""Tests for the composite-action and workflow lint script.

The shell body this script replaced lost a failure in two places: an earlier
iteration of the `action-validator` loop, and the first of the two workflow
linters. Both are ordering defects, so the tests that matter place a failing
tool first and assert the run stops there.

`cmd-mox` supplies the external executables, so no linter is installed or run
against real files.
"""

from __future__ import annotations

import typing as typ

import lint_actions
import pytest

#: cmd-mox's fixture enters the replay phase for you by default. These tests
#: declare their expectations first, so they take the lifecycle back and call
#: `replay` and `verify` themselves.
manual_lifecycle = pytest.mark.cmd_mox(auto_lifecycle=False)

if typ.TYPE_CHECKING:  # pragma: no cover - annotations only.
    from pathlib import Path

    from cmd_mox.controller import CmdMox

YAMLLINT_VERSION = "1.35.1"


@pytest.fixture
def repository(tmp_path: Path) -> Path:
    """Return a repository with one composite action and one workflow.

    Both trees are populated so the plan contains all four invocations. A
    repository with only workflows could not exhibit the loop defect at all.
    """
    action = tmp_path / ".github" / "actions" / "demo"
    action.mkdir(parents=True)
    (action / "action.yml").write_text("name: demo\n", encoding="utf-8")

    workflows = tmp_path / ".github" / "workflows"
    workflows.mkdir(parents=True)
    (workflows / "ci.yml").write_text("name: ci\n", encoding="utf-8")
    return tmp_path


def test_plan_orders_the_tools_as_the_shell_did(repository: Path) -> None:
    """The plan runs yamllint before the validator, and actions before workflows.

    Pinning the order is what lets the failure tests below claim a particular
    tool ran first, rather than merely that something failed.
    """
    tools = [
        invocation.tool
        for invocation in lint_actions.plan(repository, YAMLLINT_VERSION)
    ]

    assert tools == ["yamllint", "action-validator", "yamllint", "actionlint"]


def test_plan_is_empty_for_a_repository_with_neither_tree(tmp_path: Path) -> None:
    """Nothing to lint means nothing to run, rather than an error."""
    assert lint_actions.plan(tmp_path, YAMLLINT_VERSION) == []


def test_the_yamllint_pin_reaches_the_command(repository: Path) -> None:
    """The caller's pin must reach `uvx`, or the gate floats between versions."""
    first = lint_actions.plan(repository, YAMLLINT_VERSION)[0]

    assert first.argv[:4] == (
        "uvx",
        "--from",
        f"yamllint=={YAMLLINT_VERSION}",
        "yamllint",
    )


@manual_lifecycle
def test_a_failing_first_tool_stops_the_run(cmd_mox: CmdMox, repository: Path) -> None:
    """The first linter's failure must end the run and name that linter.

    This is the defect the shell had: `yamllint` ran first, and its status was
    replaced by whatever ran after it. Here nothing may run after it.
    """
    cmd_mox.mock("uvx").returns(exit_code=2, stderr="bad manifest")
    cmd_mox.replay()

    with pytest.raises(lint_actions.LintError) as raised:
        lint_actions.lint(repository, YAMLLINT_VERSION)

    cmd_mox.verify()
    assert raised.value.tool == "yamllint"
    assert raised.value.status == 2


@manual_lifecycle
def test_a_failing_last_tool_still_fails_the_run(
    cmd_mox: CmdMox, repository: Path
) -> None:
    """A failure in the final linter must not be lost either.

    The complement of the case above: with every earlier tool passing, the
    run's verdict has to come from the one that failed.
    """
    cmd_mox.mock("uvx").times(2).returns(exit_code=0)
    cmd_mox.mock("action-validator").returns(exit_code=0)
    cmd_mox.mock("actionlint").returns(exit_code=1, stderr="bad workflow")
    cmd_mox.replay()

    with pytest.raises(lint_actions.LintError) as raised:
        lint_actions.lint(repository, YAMLLINT_VERSION)

    cmd_mox.verify()
    assert raised.value.tool == "actionlint"
    assert raised.value.status == 1


@manual_lifecycle
def test_every_tool_passing_is_a_clean_run(cmd_mox: CmdMox, repository: Path) -> None:
    """All four invocations succeed, so the gate passes.

    Without this the failure tests above prove only that the script can raise,
    not that it distinguishes a failure from a pass.
    """
    cmd_mox.mock("uvx").times(2).returns(exit_code=0)
    cmd_mox.mock("action-validator").returns(exit_code=0)
    cmd_mox.mock("actionlint").returns(exit_code=0)
    cmd_mox.replay()

    lint_actions.lint(repository, YAMLLINT_VERSION)

    cmd_mox.verify()
