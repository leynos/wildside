"""Property tests for the nextest profile reader's ordering invariant.

The contract tests beside these assert the one workflow the repository
actually has, which is a single point in a large space.
:func:`nextest_profile.declared_profile` carries an invariant that holds
over that whole space, and it was written because a plausible simpler
version is wrong: the reader must return the *innermost* scope that
declares the variable, whatever the value found there. Deciding on the
value rather than on the key reads a step's ``NEXTEST_PROFILE: ""`` as
no declaration and reports the job's value, which is not the value
nextest receives.

The property is generated against an independent oracle rather than
against a restatement of the implementation: the oracle walks a list of
scopes the test built and decides on the key's presence in that list,
never on the value. A property that recomputed the implementation would
pass on any mutation that changed both together.

The command-position invariants of :mod:`ci_step_predicates` live in
`suite_command_property_test.py`. They were split out when this module
crossed the 400-line limit the Python lint gate enforces, on the seam
the two readers already have: this one asks which scope declares a
variable, and that one asks which command a script executes.
"""

from __future__ import annotations

import nextest_profile as profiles
from hypothesis import given
from hypothesis import strategies as st

#: Values a scope can declare, covering both blank spellings. `""` and
#: `None` are the two that a value-based reader gets wrong, so they are
#: drawn as often as ordinary names rather than left to chance.
PROFILE_VALUES = st.sampled_from(["ci", "quick", "default", "", None])

#: Whether each of step, job and workflow declares the variable.
DECLARATIONS = st.lists(st.booleans(), min_size=3, max_size=3)


def _scope(*, declares: bool, value: object) -> dict[str, object]:
    """Return one scope, with or without a `NEXTEST_PROFILE` declaration.

    Both parameters are keyword-only. `declares` is a boolean the caller
    would otherwise pass positionally, which the lint gate reads as a
    boolean trap; a suppression comment would hide the trap rather than
    remove it.

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
    scopes = [
        _scope(declares=declares, value=value)
        for declares, value in zip(declarations, values, strict=True)
    ]
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
