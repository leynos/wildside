"""Answers what one `ci.yml` step does, from the step alone.

Split from :mod:`ci_lane_reading` when that module crossed the 400-line
limit the Python lint gate enforces. The seam is the question each side
answers: :mod:`ci_lane_reading` says what the workflow declares and where
to find it, and this module says whether a given step runs the test
suite or fetches something only the suite needs.

Every question here is asked of a command at its executable position,
never of the script's text. :mod:`shell_commands` does that reading.
The distinction is the whole reason this module was reworked: matching
`cargo test` as a substring made `# cargo test`, `echo cargo test` and
`grep "cargo test"` all report an execution of the backend suite, so a
contract asserting that some step runs it would stay green after the
real invocation had been replaced by a comment.

Every function here takes a parsed step and returns a verdict. Nothing
opens a file, and nothing knows which job a step belongs to, so the same
predicate answers for the build job and the coverage job alike.
"""

from __future__ import annotations

import shell_commands as shell
from ci_lane_reading import script_of

#: Cargo subcommands that execute the test suite, as the leading
#: non-option words of a `cargo` invocation. Written as word sequences
#: rather than as strings so they are matched against the parsed command
#: and never against the script's text.
CARGO_TEST_SUBCOMMANDS: tuple[tuple[str, ...], ...] = (("nextest", "run"), ("test",))

#: Make targets that execute the Rust suite. Matched as whole target
#: tokens, never as a substring: `make test-workflow-contracts` contains
#: "make test" and runs no Rust at all, so a substring search would
#: report the Python contract gate as a duplicate backend lane.
MAKE_TEST_TARGETS = ("test", "test-rust")

#: Shells that execute the script named as their first argument, so that
#: `bash scripts/warm-pg-embedded-cache.sh` runs the script rather than
#: merely mentioning it.
SHELL_INTERPRETERS = frozenset({"bash", "dash", "sh", "zsh"})


def _arguments(command: list[str]) -> list[str]:
    """Return the non-option words a command passes its executable.

    Both `-` and `+` introduce an option word: cargo spells a toolchain
    override as `+nightly`, which is not a subcommand.

    Parameters
    ----------
    command : list[str]
        One executed command, executable first.

    Returns
    -------
    list[str]
        The remaining words that are not options.
    """
    return [word for word in command[1:] if not word.startswith(("-", "+"))]


def _runs_cargo_tests(command: list[str]) -> bool:
    """Return whether a cargo invocation executes the test suite.

    Parameters
    ----------
    command : list[str]
        One executed command whose executable is cargo.

    Returns
    -------
    bool
        True when the leading non-option words name a test subcommand.
    """
    words = _arguments(command)
    return any(
        tuple(words[: len(subcommand)]) == subcommand
        for subcommand in CARGO_TEST_SUBCOMMANDS
    )


def _targets_of_one_command(command: list[str]) -> set[str]:
    """Return the Make targets one executed command names.

    A command whose executable is not `make` names none. Otherwise every
    non-option word is a target, except a `NAME=value` variable override,
    which sets a variable rather than naming a target.

    Parameters
    ----------
    command : list[str]
        One executed command, executable first.

    Returns
    -------
    set[str]
        The targets named, whole.
    """
    if shell.executable_name(command) != "make":
        return set()
    return {word for word in _arguments(command) if "=" not in word}


def make_targets(script: str) -> set[str]:
    """Return the Make targets a script invokes.

    Only a `make` at a command's executable position claims targets, so
    `echo make test` names none. Tokens are compared whole, so
    `make test-workflow-contracts` yields one target and never answers a
    question about `test`.

    Parameters
    ----------
    script : str
        A step's `run` value.

    Returns
    -------
    set[str]
        Every target named on a `make` invocation.

    Examples
    --------
    >>> sorted(make_targets("make test-rust"))
    ['test-rust']
    >>> sorted(make_targets("make test-workflow-contracts"))
    ['test-workflow-contracts']
    >>> sorted(make_targets("make lint test && echo done"))
    ['lint', 'test']
    >>> sorted(make_targets("echo make test"))
    []
    >>> sorted(make_targets("# make test"))
    []
    """
    return {
        target
        for words in shell.command_words(script)
        for target in _targets_of_one_command(shell.executed_command(words))
    }


def runs_the_suite(step: dict[str, object]) -> bool:
    """Return whether a step executes the Rust test suite.

    Both routes are read. A cargo invocation names the runner directly;
    a `make` invocation names a target that wraps one, and `make test`
    reaches `test-rust` and so the whole suite. Either way the word has
    to stand at a command's executable position: a step that only prints
    or comments the command runs nothing.

    Parameters
    ----------
    step : dict[str, object]
        The parsed step.

    Returns
    -------
    bool
        True when the step runs the suite by either route.

    Examples
    --------
    >>> runs_the_suite({"run": "cargo nextest run --locked"})
    True
    >>> runs_the_suite({"run": 'RUSTFLAGS="-D warnings" cargo test'})
    True
    >>> runs_the_suite({"run": "make test-rust"})
    True
    >>> runs_the_suite({"run": "echo cargo test"})
    False
    >>> runs_the_suite({"run": "# cargo test"})
    False
    >>> runs_the_suite({"uses": "actions/checkout@v4"})
    False
    """
    for words in shell.command_words(script_of(step) or ""):
        command = shell.executed_command(words)
        name = shell.executable_name(command)
        if name == "cargo" and _runs_cargo_tests(command):
            return True
        if _targets_of_one_command(command) & set(MAKE_TEST_TARGETS):
            return True
    return False


#: Make targets that acquire test tooling, named as targets rather than
#: as tools: a step name is prose and an installed tool leaves no trace
#: in the workflow, so the invocation is the thing that has to be found.
TOOLING_TARGETS = ("prepare-pg-worker",)

#: Scripts that acquire test tooling, by the path the workflow runs.
TOOLING_SCRIPTS = ("scripts/warm-pg-embedded-cache.sh",)

#: The installer action, and the tool prefixes it may be asked for in the
#: build job. `taiki-e/install-action` names its tool in an input rather
#: than in a script, so a script search cannot see it.
INSTALL_ACTION = "taiki-e/install-action"
INSTALL_ACTION_TEST_TOOLS = ("nextest",)


def _runs_a_tooling_script(command: list[str]) -> bool:
    """Return whether an executed command runs a tooling script.

    A script runs either as the executable itself or as the first
    argument to a shell, which is how this workflow spells it.

    Parameters
    ----------
    command : list[str]
        One executed command, executable first.

    Returns
    -------
    bool
        True when the command executes one of :data:`TOOLING_SCRIPTS`.
    """
    if not command:
        return False
    candidates = [command[0]]
    if shell.executable_name(command) in SHELL_INTERPRETERS:
        candidates += _arguments(command)[:1]
    return any(
        candidate.endswith(script)
        for candidate in candidates
        for script in TOOLING_SCRIPTS
    )


def _installs_a_test_tool(step: dict[str, object]) -> bool:
    """Return whether a step asks the installer action for a test tool.

    Parameters
    ----------
    step : dict[str, object]
        The parsed step.

    Returns
    -------
    bool
        True when the step uses the installer action and names one of
        the test tools in its `tool` input.
    """
    if INSTALL_ACTION not in str(step.get("uses", "")):
        return False
    options = step.get("with")
    if not isinstance(options, dict):
        return False
    requested = str(options.get("tool", ""))
    return any(requested.startswith(tool) for tool in INSTALL_ACTION_TEST_TOOLS)


def acquires_test_tooling(step: dict[str, object]) -> bool:
    """Return whether a step obtains something only the suite needs.

    Both routes are read, because a tool arrives either by a command in
    a script or by an input to the installer action, and a search that
    knew only one would miss the other. A command in a script is read at
    its executable position, for the same reason :func:`runs_the_suite`
    is.

    Parameters
    ----------
    step : dict[str, object]
        The parsed step.

    Returns
    -------
    bool
        True when the step fetches the worker binary, warms the database
        archive, or installs a test runner.

    Examples
    --------
    >>> acquires_test_tooling({"run": "make prepare-pg-worker"})
    True
    >>> acquires_test_tooling({"run": "bash scripts/warm-pg-embedded-cache.sh"})
    True
    >>> acquires_test_tooling({"run": "make lint-clippy"})
    False
    >>> acquires_test_tooling({"run": "echo make prepare-pg-worker"})
    False
    >>> acquires_test_tooling(
    ...     {"uses": "taiki-e/install-action@abc", "with": {"tool": "nextest@1"}}
    ... )
    True
    """
    for words in shell.command_words(script_of(step) or ""):
        command = shell.executed_command(words)
        if _targets_of_one_command(command) & set(TOOLING_TARGETS):
            return True
        if _runs_a_tooling_script(command):
            return True
    return _installs_a_test_tool(step)
