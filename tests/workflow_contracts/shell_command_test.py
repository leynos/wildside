"""Behavioural cases for reading a command at its executable position.

The property tests beside these generate scripts from a fixed vocabulary
and compare two tokenizers. They cannot cover the spellings a generator
would have to be told about one at a time, and those spellings are
exactly where the first version of the reader was wrong: it matched
`cargo test` as a substring, so a comment, an `echo` and a quoted string
each reported that the step ran the backend suite.

The two directions are not equally dangerous, and the cases are grouped
to say so:

- A false positive is silent. A contract asserting that some build step
  runs the suite stays green after the real invocation is replaced by a
  diagnostic `echo` or commented out, which is the regression the lane
  contracts exist to catch.
- A false negative is loud. The lane contracts assert that the suite is
  run *somewhere*, so a spelling the reader fails to recognize fails
  those contracts rather than passing them vacuously.

Step shapes are covered here too. `run` is absent on every step that
uses an action, and a step whose `run` is not a string is a malformed
workflow rather than a step that runs the suite; both have to reach a
verdict rather than an exception.
"""

from __future__ import annotations

import ci_step_predicates as does
import pytest
import shell_commands as shell

#: Spellings that mention a suite command without executing one. Every
#: one of these was true under the substring reader this replaced.
MENTIONS_ONLY = [
    pytest.param("# cargo test", id="cargo-in-a-comment"),
    pytest.param("# make test", id="make-in-a-comment"),
    pytest.param("make lint  # make test", id="comment-after-a-command"),
    pytest.param("# make test && make test", id="comment-swallows-the-chain"),
    pytest.param("echo cargo test", id="cargo-echoed"),
    pytest.param("echo make test", id="make-echoed"),
    pytest.param('echo "cargo nextest run"', id="cargo-echoed-quoted"),
    pytest.param('grep "cargo test" ci.yml', id="cargo-as-a-search-pattern"),
    pytest.param("printf 'make test'", id="make-in-a-format-string"),
    pytest.param("echo remake tested", id="lookalike-words"),
    pytest.param("make test-frontend", id="make-target-sharing-a-prefix"),
    pytest.param("make test-workflow-contracts", id="the-python-contract-gate"),
    pytest.param("cargo build --tests", id="cargo-build-not-test"),
    pytest.param("cargo install nextest", id="installing-the-runner"),
]

#: Spellings that do execute the suite. A reader that demanded the bare
#: word `cargo` at position zero would refuse every one of these.
EXECUTES_THE_SUITE = [
    pytest.param("cargo test", id="plain"),
    pytest.param("cargo nextest run --locked", id="through-nextest"),
    pytest.param("cargo +nightly test", id="with-a-toolchain-override"),
    pytest.param("cargo --locked test", id="with-an-option-before-the-subcommand"),
    pytest.param("/usr/bin/cargo test", id="path-qualified"),
    pytest.param('RUSTFLAGS="-D warnings" cargo test', id="behind-an-assignment"),
    pytest.param("env cargo nextest run", id="behind-env"),
    pytest.param("timeout 600 cargo test", id="behind-timeout"),
    pytest.param("nice -n 5 make test", id="behind-nice"),
    pytest.param("env RUSTFLAGS=-D make test-rust", id="behind-env-and-an-assignment"),
    pytest.param("make lint && make test", id="second-in-a-chain"),
    pytest.param("make lint\nmake test-rust", id="second-on-its-own-line"),
    pytest.param("cargo nextest run \\\n  --locked", id="across-a-continuation"),
    pytest.param("make -j4 test", id="with-a-make-option"),
    pytest.param("make VAR=1 test", id="with-a-variable-override"),
]

#: Step shapes that name no script at all. Each must reach a verdict.
NO_SCRIPT = [
    pytest.param({}, id="empty-step"),
    pytest.param({"uses": "actions/checkout@v4"}, id="an-action-step"),
    pytest.param({"run": None}, id="a-valueless-run-key"),
    pytest.param({"run": 12}, id="a-run-value-that-is-not-a-string"),
    pytest.param({"run": ["cargo", "test"]}, id="a-run-value-that-is-a-list"),
    pytest.param({"run": ""}, id="an-empty-script"),
    pytest.param({"run": 'echo "unbalanced'}, id="an-unbalanced-quote"),
]


@pytest.mark.parametrize("script", MENTIONS_ONLY)
def test_a_mentioned_command_does_not_run_the_suite(script: str) -> None:
    """Naming a suite command without executing it is not running it.

    Scenario: the command appears in a comment, in `echo` output, in a
    quoted argument or as a lookalike target. Invariant: the verdict is
    false. This is the silent direction: a contract that required some
    step to run the suite would stay green with the real invocation
    commented out, which is the regression the lane contracts exist to
    catch.
    """
    assert does.runs_the_suite({"run": script}) is False, (
        f"{script!r} mentions a suite command but executes none"
    )


@pytest.mark.parametrize("script", EXECUTES_THE_SUITE)
def test_an_executed_command_runs_the_suite(script: str) -> None:
    """Every spelling that executes the suite is read as running it.

    Scenario: the invocation stands behind an environment assignment, a
    transparent wrapper, a path-qualified executable, a toolchain
    override, a line continuation or another command in a chain.
    Invariant: the verdict is true. A reader that demanded the bare word
    at position zero would refuse the form `ci.yml` actually uses,
    `RUSTFLAGS="-D warnings" cargo nextest run`.
    """
    assert does.runs_the_suite({"run": script}) is True, (
        f"{script!r} executes the suite"
    )


@pytest.mark.parametrize("step", NO_SCRIPT)
def test_a_step_with_no_readable_script_runs_nothing(step: dict[str, object]) -> None:
    """A step without a readable script reaches a verdict, not an error.

    Scenario: the step uses an action, declares a valueless `run`, or
    declares one whose value is not a string. Invariant: both predicates
    return false. A reader that indexed into the value or lexed it
    regardless would raise here, and a contract cannot report on a
    workflow it cannot finish reading.
    """
    assert does.runs_the_suite(step) is False, f"{step!r} runs no suite"
    assert does.acquires_test_tooling(step) is False, f"{step!r} acquires nothing"


@pytest.mark.parametrize(
    "script",
    [
        pytest.param("echo make prepare-pg-worker", id="target-echoed"),
        pytest.param("# make prepare-pg-worker", id="target-in-a-comment"),
        pytest.param("echo scripts/warm-pg-embedded-cache.sh", id="script-path-echoed"),
        pytest.param(
            "# bash scripts/warm-pg-embedded-cache.sh", id="script-in-a-comment"
        ),
        pytest.param("make prepare-pg-worker-docs", id="a-lookalike-target"),
    ],
)
def test_a_mentioned_tooling_command_acquires_nothing(script: str) -> None:
    """Tooling is read at an executable position for the same reason.

    Scenario: the worker-binary target or the cache warm-up script is
    named but not executed. Invariant: the verdict is false. The guard
    contracts require every tooling step to carry the Dependabot
    condition, and a step that merely mentions the command is not a step
    that needs the guard.
    """
    assert does.acquires_test_tooling({"run": script}) is False, (
        f"{script!r} mentions tooling but acquires none"
    )


@pytest.mark.parametrize(
    "script",
    [
        pytest.param("make prepare-pg-worker", id="the-make-target"),
        pytest.param(
            "bash scripts/warm-pg-embedded-cache.sh", id="the-script-through-bash"
        ),
        pytest.param(
            "./scripts/warm-pg-embedded-cache.sh", id="the-script-executed-directly"
        ),
        pytest.param("sudo make prepare-pg-worker", id="the-target-behind-a-wrapper"),
    ],
)
def test_an_executed_tooling_command_acquires_tooling(script: str) -> None:
    """Every spelling that fetches test tooling is read as fetching it.

    Scenario: the target runs directly or behind a wrapper, and the
    warm-up script runs through `bash` or as the executable itself.
    Invariant: the verdict is true. `ci.yml` spells the second one
    `bash scripts/warm-pg-embedded-cache.sh`, so a reader that knew only
    the direct form would report the warm-up as unguarded tooling-free
    work.
    """
    assert does.acquires_test_tooling({"run": script}) is True, (
        f"{script!r} acquires test tooling"
    )


def test_an_unbalanced_quote_yields_no_command() -> None:
    """A line that cannot be lexed contributes no command.

    Scenario: a `run` block with an unbalanced quote, which is a broken
    workflow rather than a lane. Invariant: the reader returns the
    commands it could read and does not raise. A shape fault belongs to
    the workflow reader, and a predicate that raised here would stop a
    contract before it reported the fault it was looking for.
    """
    assert shell.command_words('echo "unbalanced\nmake test') == [["make", "test"]], (
        "the unreadable line contributes nothing and the next line still reads"
    )
