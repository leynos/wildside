"""Behavioural cases for reading a command out of a shell line.

`shell_invocations` backs the rule that no pull-request workflow may run
the CodeScene CLI. A rule is only as good as the reading behind it: every
spelling the reader fails to recognize is a way to run the command while
the contract reports that nothing ran, and a rule satisfied by a spelling
is worked around by choosing that spelling rather than by fixing the
workflow.

The cases are grouped by the reading each one exercises, and every
positive case here is a form that the first version of this reader
answered `False` for.
"""

from __future__ import annotations

import pytest
import shell_invocations as shell

CLI = "cs-coverage"

#: Lines that do run the CLI. Each names a different way the executable
#: fails to be the line's first word.
RUNS_THE_CLI = [
    pytest.param("cs-coverage check --format lcov", id="plain"),
    pytest.param("/opt/codescene/bin/cs-coverage check", id="path-qualified"),
    pytest.param("./tools/cs-coverage upload", id="relative-path"),
    pytest.param("CS_TOKEN=x cs-coverage check", id="behind-an-assignment"),
    pytest.param("CS_TOKEN=x PATH=/opt/bin cs-coverage check", id="two-assignments"),
    pytest.param("env cs-coverage check", id="behind-env"),
    pytest.param("env -u HOME cs-coverage check", id="behind-env-unsetting"),
    pytest.param("env --chdir=/tmp cs-coverage check", id="behind-env-with-equals"),
    pytest.param("sudo cs-coverage check", id="behind-sudo"),
    pytest.param("sudo -u runner cs-coverage check", id="behind-sudo-as-a-user"),
    pytest.param("exec -a name cs-coverage check", id="behind-exec"),
    pytest.param("time -f %e cs-coverage check", id="behind-time-with-a-format"),
    pytest.param("env CS_TOKEN=x cs-coverage check", id="env-then-an-assignment"),
    pytest.param("echo preparing && cs-coverage check", id="second-in-an-and-list"),
    pytest.param("true; cs-coverage check", id="second-in-a-semicolon-list"),
    pytest.param("cs-coverage check || echo failed", id="first-in-an-or-list"),
    pytest.param("false || sudo -u runner cs-coverage check", id="wrapped-and-listed"),
]

#: Lines that name the CLI without running it. Every one of these would
#: be a false positive, and a false positive here is loud rather than
#: silent: it fails a contract on a workflow that does nothing wrong.
DOES_NOT_RUN_THE_CLI = [
    pytest.param("echo cs-coverage check", id="echoed"),
    pytest.param('echo "cs-coverage check"', id="echoed-quoted"),
    pytest.param("cat docs/cs-coverage-notes.md", id="a-path-containing-the-name"),
    pytest.param("grep cs-coverage .github/workflows/ci.yml", id="a-search-pattern"),
    pytest.param("cs-coverage-notes --print", id="a-longer-executable-name"),
    pytest.param("make cs-coverage", id="a-make-target-of-that-name"),
    pytest.param('echo "a && cs-coverage check"', id="a-quoted-list"),
    pytest.param("echo 'unbalanced", id="an-unbalanced-quote"),
    pytest.param("", id="an-empty-line"),
]


@pytest.mark.parametrize("line", RUNS_THE_CLI)
def test_a_line_that_runs_the_cli_is_found(line: str) -> None:
    """Every spelling that executes the CLI is read as executing it.

    Scenario: the invocation stands behind an assignment, behind a
    wrapper whose option takes an operand, behind a path, or after
    another command in a shell list. Invariant: the verdict is true.

    The wrapper operands are the subtle half. `sudo -u runner
    cs-coverage check` puts the executable three words in, and a reader
    that skipped only the option word would read `runner` as the
    command and report that nothing ran.
    """
    assert shell.invokes(line, CLI) is True, f"{line!r} runs {CLI}"


@pytest.mark.parametrize("line", DOES_NOT_RUN_THE_CLI)
def test_a_line_that_only_names_the_cli_is_not_found(line: str) -> None:
    """Naming the CLI without executing it is not executing it.

    Scenario: the name appears as an argument, inside quotes, as part of
    a longer path or executable name, or in a line that cannot be lexed.
    Invariant: the verdict is false. A reader that matched the text
    would fail a contract on a workflow that does nothing wrong, and the
    fix for that failure would be to rename a file.
    """
    assert shell.invokes(line, CLI) is False, f"{line!r} does not run {CLI}"


@pytest.mark.parametrize(
    ("line", "expected"),
    [
        pytest.param("cs coverage upload", True, id="the-subcommand"),
        pytest.param("true && cs coverage upload", True, id="the-subcommand-listed"),
        pytest.param("sudo -u runner cs coverage upload", True, id="wrapped"),
        pytest.param("cs rules-config validate", False, id="another-subcommand"),
        pytest.param("cs", False, id="no-subcommand-at-all"),
        pytest.param("echo cs coverage", False, id="echoed"),
    ],
)
def test_a_required_subcommand_is_compared_whole(line: str, *, expected: bool) -> None:
    """The `cs` binary counts only with its `coverage` subcommand.

    Scenario: the `cs` binary with the subcommand, with another
    subcommand, with none, and echoed. Invariant: the verdict is true
    only for the required subcommand, after the same wrapper and list
    handling as the standalone spelling.

    `cs` alone is a different tool with other subcommands, one of which
    this repository uses for rule-set validation, so matching the binary
    without the subcommand would forbid a command the guide recommends.
    """
    assert shell.invokes(line, "cs", "coverage") is expected, (
        f"{line!r} should read as {expected}"
    )
