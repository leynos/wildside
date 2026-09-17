"""Answers what one `ci.yml` step does, from the step alone.

Split from :mod:`ci_lane_reading` when that module crossed the 400-line
limit the Python lint gate enforces. The seam is the question each side
answers: :mod:`ci_lane_reading` says what the workflow declares and where
to find it, and this module says whether a given step runs the test
suite or fetches something only the suite needs.

Every function here takes a parsed step and returns a verdict. Nothing
opens a file, and nothing knows which job a step belongs to, so the same
predicate answers for the build job and the coverage job alike.
"""

from __future__ import annotations

from ci_lane_reading import script_of

#: Cargo invocations that execute the test suite.
CARGO_TEST_INVOCATIONS = ("cargo nextest run", "cargo test")

#: Make targets that execute the Rust suite. Matched as whole target
#: tokens, never as a substring: `make test-workflow-contracts` contains
#: "make test" and runs no Rust at all, so a substring search would
#: report the Python contract gate as a duplicate backend lane.
MAKE_TEST_TARGETS = ("test", "test-rust")


def _targets_on_one_line(line: str) -> set[str]:
    """Return the Make targets one command line names.

    A line without a `make` word names none. Otherwise every word after
    the first `make` is a target, except option words, which begin with
    a dash.

    Parameters
    ----------
    line : str
        One command line, already split on shell separators.

    Returns
    -------
    set[str]
        The targets named, whole.
    """
    words = line.split()
    if "make" not in words:
        return set()
    return {
        word for word in words[words.index("make") + 1 :] if not word.startswith("-")
    }


def make_targets(script: str) -> set[str]:
    """Return the Make targets a script invokes.

    Each `make` word claims the words after it on its own line as
    targets, stopping at a shell separator. Tokens are compared whole,
    so `make test-workflow-contracts` yields one target and never
    answers a question about `test`.

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
    """
    separated = script.replace("&&", "\n").replace(";", "\n")
    return {
        target
        for line in separated.splitlines()
        for target in _targets_on_one_line(line)
    }


def runs_the_suite(step: dict[str, object]) -> bool:
    """Return whether a step executes the Rust test suite.

    Both routes are read. A cargo invocation names the runner directly;
    a `make` invocation names a target that wraps one, and `make test`
    reaches `test-rust` and so the whole suite.

    Parameters
    ----------
    step : dict[str, object]
        The parsed step.

    Returns
    -------
    bool
        True when the step runs the suite by either route.
    """
    script = script_of(step) or ""
    if any(command in script for command in CARGO_TEST_INVOCATIONS):
        return True
    return bool(make_targets(script) & set(MAKE_TEST_TARGETS))


#: Commands that acquire test tooling, each named as the command rather
#: than as the tool. A step name is prose and an installed tool leaves no
#: trace in the workflow; the command is the thing that has to be found.
TOOLING_COMMANDS = (
    "make prepare-pg-worker",
    "scripts/warm-pg-embedded-cache.sh",
)

#: The installer action, and the tool prefixes it may be asked for in the
#: build job. `taiki-e/install-action` names its tool in an input rather
#: than in a script, so a script search cannot see it.
INSTALL_ACTION = "taiki-e/install-action"
INSTALL_ACTION_TEST_TOOLS = ("nextest",)


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
    knew only one would miss the other.

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
    >>> acquires_test_tooling({"run": "make lint-clippy"})
    False
    >>> acquires_test_tooling(
    ...     {"uses": "taiki-e/install-action@abc", "with": {"tool": "nextest@1"}}
    ... )
    True
    """
    script = script_of(step) or ""
    if any(command in script for command in TOOLING_COMMANDS):
        return True
    return _installs_a_test_tool(step)
