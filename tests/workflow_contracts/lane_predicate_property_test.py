"""Property tests for the lane readers' ordering and token invariants.

The contract tests beside these assert the one workflow the repository
actually has, which is a single point in a large space. Two of the new
readers carry invariants that hold over that whole space, and both were
written because a plausible simpler version is wrong:

- :func:`nextest_profile.declared_profile` must return the *innermost*
  scope that declares the variable, whatever the value found there.
  Deciding on the value rather than on the key reads a step's
  ``NEXTEST_PROFILE: ""`` as no declaration and reports the job's value,
  which is not the value nextest receives.
- :func:`ci_step_predicates.make_targets` must compare whole target
  tokens. A substring search reports ``make test-workflow-contracts``,
  which runs no Rust, as an execution of the Rust suite.

Both are generated against an independent oracle rather than against a
restatement of the implementation. The profile oracle walks a list of
scopes built by the test; the Make oracle tokenizes with a different
strategy, splitting the whole script at once rather than line by line.
A property that recomputed the implementation would pass on any mutation
that changed both together.
"""

from __future__ import annotations

import re
import shlex
from itertools import starmap

import ci_step_predicates as does
import nextest_profile as profiles
from hypothesis import given
from hypothesis import strategies as st

#: Values a scope can declare, covering both blank spellings. `""` and
#: `None` are the two that a value-based reader gets wrong, so they are
#: drawn as often as ordinary names rather than left to chance.
PROFILE_VALUES = st.sampled_from(["ci", "quick", "default", "", None])

#: Whether each of step, job and workflow declares the variable.
DECLARATIONS = st.lists(st.booleans(), min_size=3, max_size=3)

#: Words that can appear in a command line around a `make` invocation.
COMMAND_WORDS = st.sampled_from([
    "make",
    "test",
    "test-rust",
    "test-frontend",
    "test-workflow-contracts",
    "lint",
    "-j4",
    "--silent",
    "cargo",
    "echo",
    "testing",
    "rust-test",
])

SEPARATORS = st.sampled_from(["\n", " && ", "; "])

#: One command: words joined by spaces, so a `make` word is followed by
#: its targets in the same chunk. Joining every word with a separator
#: instead puts each word in a chunk of its own, and then no script ever
#: names a target at all: the first version of these properties did that
#: and passed under a mutation that emitted target prefixes.
COMMANDS = st.lists(COMMAND_WORDS, min_size=1, max_size=4).map(" ".join)

#: A script: commands joined by shell separators.
SCRIPTS = st.lists(COMMANDS, min_size=1, max_size=4).flatmap(
    lambda commands: SEPARATORS.map(lambda sep: sep.join(commands))
)


def _scope(declares: bool, value: object) -> dict[str, object]:  # noqa: FBT001
    """Return one scope, with or without a `NEXTEST_PROFILE` declaration.

    Parameters
    ----------
    declares : bool
        Whether this scope declares the variable at all.
    value : object
        The value to declare, ignored when `declares` is false.

    Returns
    -------
    dict[str, object]
        A parsed scope carrying an `env` mapping.
    """
    environment: dict[str, object] = {"RUST_BACKTRACE": "1"}
    if declares:
        environment[profiles.PROFILE_VARIABLE] = value
    return {"env": environment}


@given(
    declarations=DECLARATIONS, values=st.lists(PROFILE_VALUES, min_size=3, max_size=3)
)
def test_the_innermost_declaring_scope_wins(
    declarations: list[bool], values: list[object]
) -> None:
    """The first scope that declares the variable is the one reported.

    Scenario: step, job and workflow each may or may not declare
    `NEXTEST_PROFILE`, and a declared value may be an ordinary name, the
    empty string or `None`. Invariant: the reader names the innermost
    declaring scope and returns exactly the value found there, and
    reports absence only when no scope declares it.

    The oracle walks the same three scopes in the same order but decides
    on the key's presence in a list the test built, never on the value.
    A reader that fell through on a blank value disagrees with it on
    exactly the `""` and `None` draws.
    """
    scopes = list(starmap(_scope, zip(declarations, values, strict=True)))
    step, job, document = scopes

    found = profiles.declared_profile(document, job, step)

    expected = [
        (name, value)
        for name, declares, value in zip(
            profiles.SCOPE_NAMES, declarations, values, strict=True
        )
        if declares
    ]
    if not expected:
        assert found.is_absent(), (
            f"no scope declares {profiles.PROFILE_VARIABLE}, so the reader must "
            f"report absence, not {found!r}"
        )
        return
    assert not found.is_absent(), (
        f"{expected[0][0]} declares {profiles.PROFILE_VARIABLE}, so the reader "
        "must not report absence"
    )
    assert (found.scope, found.value) == expected[0], (
        f"expected the innermost declaring scope {expected[0]!r}, got "
        f"{(found.scope, found.value)!r}"
    )


#: Shell separators between commands, as one pattern. The implementation
#: reaches the same split by chained `str.replace` and `str.splitlines`;
#: this is deliberately a different mechanism, so the two agree only when
#: both are right rather than when both share a bug.
_SEPARATOR = re.compile(r"&&|[;\n]")


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


def _oracle_targets(script: str) -> set[str]:
    """Return the Make targets a script names, tokenized independently.

    Splits with a regular expression and words with :mod:`shlex`, where
    the implementation uses `str.replace`, `str.splitlines` and
    `str.split`. Neither step shares code with what it checks.

    Parameters
    ----------
    script : str
        The command text.

    Returns
    -------
    set[str]
        Whole target tokens.
    """
    targets: set[str] = set()
    for chunk in _SEPARATOR.split(script):
        words = shlex.split(chunk)
        if "make" not in words:
            continue
        after = words[words.index("make") + 1 :]
        targets |= {word for word in after if not word.startswith("-")}
    return targets


@given(script=SCRIPTS)
def test_make_targets_agrees_with_an_independent_tokenizer(script: str) -> None:
    """Targets are whole tokens however the command is spelled.

    Scenario: a script names Make targets across lines, `&&` chains and
    `;` chains, mixed with option words and unrelated commands.
    Invariant: the reader's targets equal an independently tokenized
    oracle's. Both refuse option words and neither splits a token, so a
    reader that matched substrings disagrees wherever a lookalike target
    such as `test-frontend` appears.
    """
    assert does.make_targets(script) == _oracle_targets(script), (
        f"tokenization disagreed on {script!r}"
    )


@given(script=SCRIPTS)
def test_only_the_named_make_targets_run_the_suite(script: str) -> None:
    """Only exact `test` and `test-rust` targets count as the suite.

    Scenario: the same generated scripts, read for whether they execute
    the Rust suite. Invariant: the verdict is true exactly when a whole
    target token equals one of the two named targets. `test-frontend`
    and `test-workflow-contracts` share a prefix with `test` and run no
    Rust, so a prefix or substring rule makes this false.

    Both routes are expected, not just the Make one. The first version
    of this property compared against the Make targets alone and was
    falsified immediately by `cargo test`, which is the other way a step
    runs the suite.
    """
    step: dict[str, object] = {"run": script}
    by_target = bool(_oracle_targets(script) & set(does.MAKE_TEST_TARGETS))
    by_cargo = any(command in script for command in does.CARGO_TEST_INVOCATIONS)
    expected = by_target or by_cargo
    assert does.runs_the_suite(step) is expected, (
        f"suite verdict disagreed on {script!r}: expected {expected} "
        f"(make target {by_target}, cargo {by_cargo})"
    )


@given(script=st.text(max_size=40))
def test_a_script_naming_no_make_target_never_runs_the_suite(script: str) -> None:
    """Arbitrary text without `make` or a cargo runner is not the suite.

    Scenario: free text, which is what a `run` block is. Invariant: with
    no `make` word and no cargo invocation the verdict is false. This is
    the property that would fail first if the reader ever matched a bare
    substring such as "test".
    """
    if "make" in script or any(
        command in script for command in does.CARGO_TEST_INVOCATIONS
    ):
        return
    assert does.runs_the_suite(_step(script)) is False, (
        f"{script!r} names no make target and no cargo runner"
    )
