"""Property tests for reading a command at its executable position.

:func:`ci_step_predicates.make_targets` and
:func:`ci_step_predicates.runs_the_suite` must read a command at its
executable position and compare whole target tokens. A substring search
reports ``make test-workflow-contracts``, which runs no Rust, as an
execution of the Rust suite, and reports ``echo cargo test`` and a
commented-out invocation as executions of it too.

The property is generated against an independent oracle rather than
against a restatement of the implementation. The oracle strips comments
with its own scanner, splits the remaining text with a regular
expression, words each chunk with :func:`shlex.split` and peels wrappers
by recursion, where the implementation drives :class:`shlex.shlex`
itself, cuts on the operator tokens it emits, and peels by slicing in a
loop. Its constants are restated here rather than imported, so a
mutation to either the mechanism or the data moves only one side. A
property that recomputed the implementation would pass on any mutation
that changed both together.

The oracle has already earned that independence once. Its first version
split the text into commands before removing comments, and disagreed
with the implementation on ``# make test && make test``, where the `#`
takes the rest of the line including the `&&`. The implementation was
right and the oracle was wrong, which is the result an oracle sharing
the implementation's mechanism could not have produced.

Split from `lane_predicate_property_test.py` when that module crossed
the 400-line limit the Python lint gate enforces.
"""

from __future__ import annotations

import re
import shlex
from itertools import dropwhile
from pathlib import PurePosixPath

import ci_step_predicates as does
from hypothesis import given
from hypothesis import strategies as st

#: What a command is, at its executable position. Half of these run
#: something and half only mention it, and the lookalike Make targets
#: sit beside the real ones so a prefix rule is falsified rather than
#: merely unlikely to be hit.
COMMAND_BODIES = st.sampled_from([
    "make test",
    "make test-rust",
    "make -j4 test",
    "make VAR=1 test",
    "make test-frontend",
    "make test-workflow-contracts",
    "make lint",
    "cargo test",
    "cargo test --locked -p backend",
    "cargo nextest run --locked",
    "cargo +nightly test",
    "cargo build --tests",
    "cargo install nextest",
    "/usr/bin/cargo test",
    "echo cargo test",
    "echo make test",
    "grep make test",
    "printf testing",
    "true",
])

#: What can stand in front of a command. An assignment and a wrapper are
#: transparent and must not change the verdict; `#` comments the whole
#: line out and must change it to false.
PREFIXES = st.sampled_from([
    "",
    "# ",
    "env ",
    "sudo ",
    "timeout 60 ",
    "nice -n 5 ",
    'RUSTFLAGS="-D warnings" ',
])

#: One command: a prefix and a body. The two are drawn separately so
#: every prefix meets every body, which is what makes the transparent
#: ones provably transparent and the comment provably not.
COMMANDS = st.tuples(PREFIXES, COMMAND_BODIES).map("".join)

#: Separators between commands. A `#` prefix comments out the rest of
#: its line, so a newline is drawn alongside the inline chains to keep
#: the commands after a comment reachable.
SEPARATORS = st.sampled_from(["\n", " && ", "; ", " || ", " | "])

#: A script: commands joined by shell separators.
SCRIPTS = st.lists(COMMANDS, min_size=1, max_size=4).flatmap(
    lambda commands: SEPARATORS.map(lambda sep: sep.join(commands))
)


#: Shell separators between commands, as one pattern. The implementation
#: recognises them as operator tokens that `shlex` emits with
#: `punctuation_chars`; this splits the raw text instead, so the two
#: agree only when both are right rather than when both share a bug.
#:
#: Splitting raw text cannot see quoting, so the generated scripts below
#: never put a separator inside quotes, and :func:`_strip_comment` treats
#: any unquoted `#` as a comment rather than requiring it to start a
#: word. Those are the honest limits of this oracle; the enumerated cases
#: in `shell_command_test.py` cover what the generator leaves out.
_SEPARATOR = re.compile(r"&&|\|\||[;|\n]")

#: Restated rather than imported, deliberately. An oracle that read the
#: implementation's own constants would agree with it after a mutation
#: that changed them, which is the failure these properties exist to
#: catch. Adding a wrapper or a target has to be written twice.
_WRAPPERS = frozenset({
    "command",
    "env",
    "exec",
    "nice",
    "sudo",
    "time",
    "timeout",
    "xvfb-run",
})
_SUITE_TARGETS = frozenset({"test", "test-rust"})
_CARGO_TEST_SUBCOMMANDS = (("nextest", "run"), ("test",))

_ASSIGNMENT = re.compile(r"\A[A-Za-z_][A-Za-z0-9_]*=")
_DURATION = re.compile(r"\A\d+(?:\.\d+)?[smhd]?\Z")


def _step(script: str) -> dict[str, object]:
    """Return a parsed step running one script.

    Annotated rather than written inline because `dict` is invariant in
    its value type: a `dict[str, str]` literal is not a
    `dict[str, object]`, and the type checker rejects it at the call
    site.

    Parameters
    ----------
    script : str
        The step's `run` value.

    Returns
    -------
    dict[str, object]
        The parsed step.
    """
    return {"run": script}


def _oracle_executed(words: list[str]) -> list[str]:
    """Return the command a word list executes, peeled recursively.

    The implementation peels assignments and wrappers in a loop with
    slicing; this recurses, so a mutation to either one's control flow
    does not move the other.

    Parameters
    ----------
    words : list[str]
        One command's words.

    Returns
    -------
    list[str]
        The executable and its arguments, or an empty list.
    """
    if not words:
        return []
    if _ASSIGNMENT.match(words[0]):
        return _oracle_executed(words[1:])
    if words[0] not in _WRAPPERS:
        return words
    rest = list(
        dropwhile(
            lambda word: word.startswith("-") or bool(_DURATION.match(word)),
            words[1:],
        )
    )
    return _oracle_executed(rest)


def _strip_comment(line: str) -> str:
    """Return one line up to its first unquoted `#`.

    A hand-written scanner rather than a call to :mod:`shlex`, because
    comment handling is where this oracle first disagreed with the
    implementation and an oracle that reached for the same library
    would have agreed with it wrongly. A `#` ends the line in a real
    shell, taking any `&&` after it, so the comment has to be removed
    before the line is split into commands rather than after.

    Parameters
    ----------
    line : str
        One physical line.

    Returns
    -------
    str
        The line with any comment removed.
    """
    quote = ""
    for index, char in enumerate(line):
        if quote:
            quote = "" if char == quote else quote
        elif char in "'\"":
            quote = char
        elif char == "#":
            return line[:index]
    return line


def _oracle_commands(script: str) -> list[list[str]]:
    """Return each executed command a script names, tokenized apart.

    Strips comments with its own scanner, splits the remaining text with
    a regular expression and words each chunk with :func:`shlex.split`,
    where the implementation drives :class:`shlex.shlex` itself and cuts
    on the operator tokens it emits. Neither step shares code with what
    it checks.

    Parameters
    ----------
    script : str
        The command text.

    Returns
    -------
    list[list[str]]
        One entry per executed command, wrappers and assignments gone.
    """
    commands: list[list[str]] = []
    stripped = "\n".join(
        _strip_comment(line) for line in script.replace("\\\n", " ").splitlines()
    )
    for chunk in _SEPARATOR.split(stripped):
        try:
            words = shlex.split(chunk)
        except ValueError:
            continue
        executed = _oracle_executed(words)
        if executed:
            commands.append(executed)
    return commands


def _oracle_targets(script: str) -> set[str]:
    """Return the Make targets a script names, tokenized independently.

    Only a command whose executable is `make` claims targets, so an
    echoed or commented invocation names none.

    Parameters
    ----------
    script : str
        The command text.

    Returns
    -------
    set[str]
        Whole target tokens.
    """
    return {
        word
        for command in _oracle_commands(script)
        if PurePosixPath(command[0]).name == "make"
        for word in command[1:]
        if not word.startswith(("-", "+")) and "=" not in word
    }


def _oracle_runs_cargo_tests(script: str) -> bool:
    """Return whether a script executes cargo's test runner.

    Parameters
    ----------
    script : str
        The command text.

    Returns
    -------
    bool
        True when a command's executable is cargo and its leading
        non-option words name a test subcommand.
    """
    for command in _oracle_commands(script):
        if PurePosixPath(command[0]).name != "cargo":
            continue
        words = [word for word in command[1:] if not word.startswith(("-", "+"))]
        if any(
            tuple(words[: len(subcommand)]) == subcommand
            for subcommand in _CARGO_TEST_SUBCOMMANDS
        ):
            return True
    return False


@given(script=SCRIPTS)
def test_make_targets_agrees_with_an_independent_tokenizer(script: str) -> None:
    """Targets are whole tokens at an executable position, however spelled.

    Scenario: a script names Make targets across lines, `&&` chains and
    `;` chains, mixed with option words, wrappers, comment lines and
    `echo` prefixes. Invariant: the reader's targets equal an
    independently tokenized oracle's. Both refuse option words, neither
    splits a token, and neither counts a `make` word that is an argument
    rather than a command, so a reader that matched substrings or that
    read any `make` word disagrees wherever `echo make test` appears.
    """
    assert does.make_targets(script) == _oracle_targets(script), (
        f"tokenization disagreed on {script!r}"
    )


@given(script=SCRIPTS)
def test_only_an_executed_suite_command_runs_the_suite(script: str) -> None:
    """The verdict is true exactly when a command executes the suite.

    Scenario: the same generated scripts, read for whether they execute
    the Rust suite. Invariant: the verdict is true exactly when some
    executed command is `make` with a whole `test` or `test-rust`
    target, or cargo with a test subcommand. `test-frontend` and
    `test-workflow-contracts` share a prefix with `test` and run no
    Rust, so a prefix or substring rule makes this false; and
    `echo cargo test` executes nothing, so a text search makes it false
    the other way.

    Both routes are expected, not just the Make one. The first version
    of this property compared against the Make targets alone and was
    falsified immediately by `cargo test`, which is the other way a step
    runs the suite.
    """
    by_target = bool(_oracle_targets(script) & _SUITE_TARGETS)
    by_cargo = _oracle_runs_cargo_tests(script)
    expected = by_target or by_cargo
    assert does.runs_the_suite(_step(script)) is expected, (
        f"suite verdict disagreed on {script!r}: expected {expected} "
        f"(make target {by_target}, cargo {by_cargo})"
    )


@given(script=st.text(max_size=40))
def test_a_script_executing_no_suite_command_never_runs_the_suite(
    script: str,
) -> None:
    """Arbitrary text executing no suite command is not the suite.

    Scenario: free text, which is what a `run` block is. Invariant: with
    no executed `make` suite target and no executed cargo test command
    the verdict is false. This is the property that would fail first if
    the reader ever matched a bare substring such as "test", and the
    oracle it defers to is the independent one rather than a restated
    text search, so a script that merely mentions `cargo test` is
    covered here too.
    """
    if _oracle_targets(script) & _SUITE_TARGETS or _oracle_runs_cargo_tests(script):
        return
    assert does.runs_the_suite(_step(script)) is False, (
        f"{script!r} executes no make suite target and no cargo test command"
    )
