"""Tests for the composite-action and workflow lint script.

The shell body this script replaced lost a failure in two places: an earlier
iteration of the `action-validator` loop, and the first of the two workflow
linters. Both are ordering defects, so the tests that matter place a failing
tool first and assert the run stops there.

`cmd-mox` supplies the external executables, so no linter is installed or run
against real files.
"""

from __future__ import annotations

import os
import typing as typ

import lint_actions
import pytest
from hypothesis import given, settings
from hypothesis import strategies as st

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
        for invocation in lint_actions.plan(
            lint_actions.discover(repository), YAMLLINT_VERSION
        )
    ]

    assert tools == ["yamllint", "action-validator", "yamllint", "actionlint"]


def test_plan_is_empty_for_a_repository_with_neither_tree(tmp_path: Path) -> None:
    """Nothing to lint means nothing to run, rather than an error."""
    assert lint_actions.plan(lint_actions.discover(tmp_path), YAMLLINT_VERSION) == []


def test_the_yamllint_pin_reaches_the_command(repository: Path) -> None:
    """The caller's pin must reach `uvx`, or the gate floats between versions."""
    first = lint_actions.plan(lint_actions.discover(repository), YAMLLINT_VERSION)[0]

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


@manual_lifecycle
def test_the_cli_exits_with_the_failing_linters_status(
    cmd_mox: CmdMox, repository: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """The command-line entry point must surface the failure, not just raise.

    Everything above tests `lint`. This tests what a caller actually invokes:
    that the process exits non-zero with the linter's own status, and says
    which linter failed. A script that detected the failure and exited zero
    would pass every test above.
    """
    cmd_mox.mock("uvx").returns(exit_code=2, stderr="bad manifest")
    cmd_mox.replay()

    with pytest.raises(SystemExit) as raised:
        lint_actions.main(yamllint_version=YAMLLINT_VERSION, repository=repository)

    cmd_mox.verify()
    assert raised.value.code == 2, "the linter's own status must reach the caller"
    assert "yamllint" in capsys.readouterr().err


@manual_lifecycle
def test_the_cli_is_silent_and_zero_when_every_linter_passes(
    cmd_mox: CmdMox, repository: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    """The companion case, so the exit status above is attributable."""
    cmd_mox.mock("uvx").times(2).returns(exit_code=0)
    cmd_mox.mock("action-validator").returns(exit_code=0)
    cmd_mox.mock("actionlint").returns(exit_code=0)
    cmd_mox.replay()

    lint_actions.main(yamllint_version=YAMLLINT_VERSION, repository=repository)

    cmd_mox.verify()
    assert capsys.readouterr().err == ""


@pytest.mark.skipif(
    os.geteuid() == 0, reason="root bypasses the directory permission this asserts"
)
def test_an_unreadable_directory_is_not_an_empty_one(repository: Path) -> None:
    """A directory the walk cannot enter must raise, not yield nothing.

    This is the defect the whole change exists to remove, one level down, and
    it is exercised against a real unreadable directory rather than a patched
    one on purpose. `Path.rglob` swallows the `OSError` and returns an empty
    list, so a test that patched `rglob` to raise would pass while the real
    code silently reported "no composite actions" for a subtree it could not
    read.
    """
    nested = repository / ".github" / "actions" / "demo"
    nested.chmod(0o000)
    try:
        with pytest.raises(lint_actions.DiscoveryError) as raised:
            lint_actions.discover(repository)
    finally:
        nested.chmod(0o755)

    assert "demo" in str(raised.value)
    assert "Permission denied" in str(raised.value)


def test_rglob_would_have_hidden_that_failure(repository: Path) -> None:
    """Pin the behaviour that made the previous implementation a no-op.

    If a future Python makes `rglob` propagate the error, the walk becomes
    belt and braces rather than load-bearing, and this test says so by
    failing.
    """
    if os.geteuid() == 0:
        pytest.skip("root bypasses the directory permission this asserts")

    nested = repository / ".github" / "actions" / "demo"
    nested.chmod(0o000)
    try:
        swallowed = list((repository / ".github" / "actions").rglob("action.yml"))
    finally:
        nested.chmod(0o755)

    assert swallowed == [], (
        "rglob no longer suppresses the traversal error; the walk in "
        "_manifests_under can be simplified"
    )


def test_a_missing_directory_is_an_empty_one(tmp_path: Path) -> None:
    """An absent directory yields nothing, which is not an error.

    The complement of the test above: a repository with no composite actions
    is normal, and must not be confused with one that cannot be read.
    """
    surfaces = lint_actions.discover(tmp_path)

    assert surfaces.manifests == ()
    assert surfaces.workflows == ()


#: Names that are valid path segments and distinct under sorting. The
#: invariants below hold for any number of surfaces, which is exactly what a
#: fixed fixture cannot show: the ordering rule is stated over an unbounded
#: set, so it is asserted over one.
_NAMES = st.text(
    alphabet=st.characters(min_codepoint=97, max_codepoint=122), min_size=1, max_size=8
)


def _build_repository(
    root: Path, manifest_names: list[str], workflow_names: list[str]
) -> None:
    """Populate ``root`` with the named composite actions and workflows."""
    for name in manifest_names:
        directory = root / ".github" / "actions" / name
        directory.mkdir(parents=True)
        (directory / "action.yml").write_text("name: a\n", encoding="utf-8")
    if workflow_names:
        workflows = root / ".github" / "workflows"
        workflows.mkdir(parents=True)
        for name in workflow_names:
            (workflows / f"{name}.yml").write_text("name: w\n", encoding="utf-8")


def _expected_tools(manifest_count: int, workflow_count: int) -> list[str]:
    """Return the tool order the gate must produce for these surface sizes."""
    expected: list[str] = []
    if manifest_count:
        expected += ["yamllint", *["action-validator"] * manifest_count]
    if workflow_count:
        expected += ["yamllint", "actionlint"]
    return expected


@given(
    manifest_names=st.lists(_NAMES, min_size=0, max_size=6, unique=True),
    workflow_names=st.lists(_NAMES, min_size=0, max_size=6, unique=True),
)
@settings(max_examples=40, deadline=None)
def test_plan_orders_every_surface_the_same_way(
    tmp_path_factory: pytest.TempPathFactory,
    manifest_names: list[str],
    workflow_names: list[str],
) -> None:
    """Actions precede workflows, and yamllint precedes each surface's linter.

    Whatever the counts, the shape is: yamllint over the manifests, one
    `action-validator` per manifest, yamllint over the workflows, then
    `actionlint`. Each group is present only when that surface has files.
    """
    root = tmp_path_factory.mktemp("repo")
    _build_repository(root, manifest_names, workflow_names)

    invocations = lint_actions.plan(lint_actions.discover(root), YAMLLINT_VERSION)

    assert [invocation.tool for invocation in invocations] == _expected_tools(
        len(manifest_names), len(workflow_names)
    )


@given(
    manifest_names=st.lists(_NAMES, min_size=1, max_size=6, unique=True),
)
@settings(max_examples=20, deadline=None)
def test_plan_visits_manifests_in_a_stable_order(
    tmp_path_factory: pytest.TempPathFactory, manifest_names: list[str]
) -> None:
    """Manifest paths are sorted, so the run order does not depend on the disk.

    Without this the ordering test above would hold while the individual
    manifests arrived in whatever order the filesystem returned them, which
    makes a failing run hard to reproduce.
    """
    root = tmp_path_factory.mktemp("repo")
    _build_repository(root, manifest_names, [])

    paths = [
        argument
        for invocation in lint_actions.plan(
            lint_actions.discover(root), YAMLLINT_VERSION
        )
        if invocation.tool == "action-validator"
        for argument in invocation.argv[1:]
    ]

    assert paths == sorted(paths)


@given(failure_index=st.integers(min_value=0, max_value=3))
@settings(max_examples=12, deadline=None)
def test_the_run_stops_at_whichever_invocation_fails(
    tmp_path_factory: pytest.TempPathFactory,
    failure_index: int,
) -> None:
    """`lint` stops at the first failing invocation, wherever it falls.

    The real loop is exercised with a substituted runner, so the property is
    about `lint`'s control flow rather than a reimplementation of it. The
    cmd-mox cases above cover the real executables; a property test should not
    stand up a shim per example.
    """
    root = tmp_path_factory.mktemp("repo")
    actions = root / ".github" / "actions" / "demo"
    actions.mkdir(parents=True)
    (actions / "action.yml").write_text("name: a\n", encoding="utf-8")
    workflows = root / ".github" / "workflows"
    workflows.mkdir(parents=True)
    (workflows / "ci.yml").write_text("name: w\n", encoding="utf-8")

    attempted: list[str] = []

    def _record(invocation: lint_actions.Invocation) -> None:
        attempted.append(invocation.tool)
        if len(attempted) == failure_index + 1:
            raise lint_actions.LintError(invocation.tool, 1)

    planned = lint_actions.plan(lint_actions.discover(root), YAMLLINT_VERSION)

    # Patched through a context rather than the `monkeypatch` fixture, which
    # Hypothesis rejects here: a function-scoped fixture is not reset between
    # generated examples.
    with pytest.MonkeyPatch.context() as patch:
        patch.setattr(lint_actions, "_run", _record)
        with pytest.raises(lint_actions.LintError) as raised:
            lint_actions.lint(root, YAMLLINT_VERSION)

    assert raised.value.tool == planned[failure_index].tool
    assert len(attempted) == failure_index + 1, (
        "no invocation after the failing one may be attempted"
    )
