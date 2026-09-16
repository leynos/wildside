"""Properties of the timeout readings, over inputs nobody wrote down.

The parameterized cases in ``timeout_reading_test.py`` pin the shapes a
configuration is known to take: the malformed ones, the boundary ones,
and the ones this repository's own files cannot exercise. They are a
table, and a table can only audit what its author thought of.

Three of the readings have a rule that holds over far more inputs than a
table can list, and each rule is stated here once and checked against
generated ones:

* the required ceiling is the sum of its three terms, so a reading that
  took the largest budget rather than their sum agrees with every
  single-step case and with nothing else;
* the watchdog in force is the innermost level that sets one, so a
  reading that stopped at the first level present, or took the last,
  agrees with every lane that sets exactly one;
* a per-test budget is a period multiplied by a count of periods, so a
  reading that ignored the multiplier agrees with every configuration
  whose multiplier is one, which is every configuration here.

The generators stay inside what the readers accept. Rejecting bad input
is the tables' job, and a property that spent its examples on values the
reader refuses would check the refusal over and over instead of the
rule.

Hypothesis runs without a deadline. These examples do no input or
output, but the suite shares a loaded host with the Rust gates, and a
per-example wall-clock limit turns that load into a failure about
nothing.
"""

from __future__ import annotations

import typing as typ

import pytest
from coverage_lanes import watchdog_of
from hypothesis import given, settings
from hypothesis import strategies as st
from nextest_budgets import largest_test_allowance
from nextest_durations import seconds
from timeout_budgets import (
    CEILING_MARGIN_SECONDS,
    WATCHDOG_VARIABLE,
    required_ceiling,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: A unit vocabulary written out here rather than imported from the
#: reader, so the property compares two independent statements of the
#: same fact. Importing the reader's own table would make every example
#: agree with itself.
UNIT_SECONDS: typ.Final[dict[str, int]] = {
    "s": 1,
    "sec": 1,
    "seconds": 1,
    "m": 60,
    "min": 60,
    "minutes": 60,
    "h": 3600,
    "hour": 3600,
    "hours": 3600,
    "d": 86400,
}

#: Whole seconds, so the arithmetic under test is exact and a failure is
#: a wrong rule rather than a rounding difference. The range reaches a
#: day, which is well above any budget a lane would sensibly declare and
#: still far from the point where a float loses whole numbers.
BUDGETS = st.integers(min_value=1, max_value=86_400).map(float)

#: The work outside the watchdogs. Zero is included: a job whose every
#: second is inside a coverage step is the case where the margin is the
#: only thing holding the ceiling above the sum.
ALLOWANCES = st.integers(min_value=0, max_value=86_400).map(float)

#: One duration span, as a count and a unit humantime accepts.
SPANS = st.tuples(
    st.integers(min_value=1, max_value=999), st.sampled_from(sorted(UNIT_SECONDS))
)

#: How many warning periods nextest waits before terminating a test.
MULTIPLIERS = st.integers(min_value=1, max_value=20)


def _span_text(spans: cabc.Sequence[tuple[int, str]]) -> str:
    """Return a humantime duration built from its spans.

    Parameters
    ----------
    spans : cabc.Sequence[tuple[int, str]]
        Count and unit pairs, in the order they should appear.

    Returns
    -------
    str
        The spans joined by single spaces, as humantime spells a
        concatenated duration.
    """
    return " ".join(f"{count}{unit}" for count, unit in spans)


def _span_seconds(spans: cabc.Sequence[tuple[int, str]]) -> int:
    """Return what those spans are worth, in whole seconds.

    Parameters
    ----------
    spans : cabc.Sequence[tuple[int, str]]
        Count and unit pairs.

    Returns
    -------
    int
        The total, computed from this module's own unit table.
    """
    return sum(count * UNIT_SECONDS[unit] for count, unit in spans)


def _profile(period: str, multiplier: int) -> str:
    """Return a nextest configuration declaring one per-test budget.

    Parameters
    ----------
    period : str
        The warning period, as humantime spells it.
    multiplier : int
        How many periods pass before nextest terminates the test.

    Returns
    -------
    str
        A configuration with a single ``[profile.default]`` table.
    """
    return (
        "[profile.default]\n"
        f'slow-timeout = {{ period = "{period}", terminate-after = {multiplier} }}\n'
    )


def _level(budget: float | None) -> dict[str, object]:
    """Return one workflow level, setting the watchdog or saying nothing.

    Parameters
    ----------
    budget : float or None
        The budget this level declares, or None for a level that
        declares none at all.

    Returns
    -------
    dict[str, object]
        A parsed workflow, job or step mapping.
    """
    if budget is None:
        return {}
    return {"env": {WATCHDOG_VARIABLE: str(int(budget))}}


@settings(deadline=None)
@given(budgets=st.lists(BUDGETS, max_size=6), allowance=ALLOWANCES)
def test_the_required_ceiling_is_its_three_terms_added(
    budgets: list[float], allowance: float
) -> None:
    """A job's ceiling has to contain every watchdog inside it.

    Scenario: a job runs the coverage action some number of times, each
    step under its own watchdog, with measured work outside them.

    Invariant: the smallest acceptable ceiling is every budget, plus
    that outside work, plus the margin. A reading that took the largest
    budget rather than the sum would agree with every one-step job,
    which is every job in this repository.
    """
    expected = sum(budgets) + allowance + CEILING_MARGIN_SECONDS
    assert required_ceiling(budgets, allowance) == pytest.approx(expected), (
        "the ceiling is the sum of all the budgets, the outside work and "
        "the margin, not a maximum over them"
    )


@settings(deadline=None)
@given(budgets=st.lists(BUDGETS, max_size=6), allowance=ALLOWANCES)
def test_the_required_ceiling_never_merely_reaches_what_it_contains(
    budgets: list[float], allowance: float
) -> None:
    """The margin is a term, not slack that can be absorbed.

    Scenario: the same job, whatever its budgets.

    Invariant: the ceiling sits strictly above the sum it contains, by
    the whole margin. A ceiling equal to that sum cancels the job at the
    moment the watchdog would have reported the overrun, and the report
    is the only thing that makes an overrun actionable.
    """
    contained = sum(budgets) + allowance
    assert required_ceiling(budgets, allowance) - contained == pytest.approx(
        CEILING_MARGIN_SECONDS
    ), "the margin is added whole, whatever the budgets below it"


@settings(deadline=None)
@given(budgets=st.lists(BUDGETS, max_size=5), extra=BUDGETS, allowance=ALLOWANCES)
def test_another_coverage_step_raises_the_required_ceiling(
    budgets: list[float], extra: float, allowance: float
) -> None:
    """A second invocation needs room of its own.

    Scenario: a job gains one more coverage step, under its own
    watchdog.

    Invariant: the required ceiling rises by exactly that step's budget.
    This is the case the arithmetic exists for and the one no workflow
    here exercises, since no job runs the action twice.
    """
    before = required_ceiling(budgets, allowance)
    after = required_ceiling([*budgets, extra], allowance)
    assert after - before == pytest.approx(extra), (
        "adding a coverage step adds its whole watchdog to the requirement"
    )


@settings(deadline=None)
@given(
    step=st.one_of(st.none(), BUDGETS),
    job=st.one_of(st.none(), BUDGETS),
    document=st.one_of(st.none(), BUDGETS),
)
def test_the_innermost_level_that_sets_a_watchdog_wins(
    step: float | None, job: float | None, document: float | None
) -> None:
    """GitHub resolves `env` innermost first, and so must the contract.

    Scenario: a watchdog is set at any combination of the step, the job
    and the workflow, including at none of them.

    Invariant: the budget in force is the innermost level that sets one,
    and None when no level does. Both workflows here set the value at
    job level, so a reading that stopped at the step would report every
    lane as unbounded and a reading that took the workflow would report
    every lane as inheriting the action's default.
    """
    expected = next(
        (value for value in (step, job, document) if value is not None), None
    )
    found = watchdog_of(_level(document), _level(job), _level(step))
    assert found == expected, (
        "the watchdog in force is the innermost level that declares one"
    )


@settings(deadline=None)
@given(
    outer=BUDGETS,
    inner=BUDGETS,
)
def test_a_blank_level_hides_the_levels_outside_it(outer: float, inner: float) -> None:
    """An inner `env` wins even when what it sets is nothing.

    Scenario: a step declares the watchdog variable as whitespace, which
    is what a workflow writes when an expression resolves to nothing,
    while its job and the workflow each declare a real budget.

    Invariant: no budget is in force. GitHub passes the step's own value
    to the action, blank and all, so the outer budgets never reach it.
    Falling through to them would report a budget the action does not
    receive, and refusing the blank outright would fail a lane GitHub
    runs happily.
    """
    step = {"env": {WATCHDOG_VARIABLE: "  "}}
    job = _level(inner)
    document = _level(outer)
    assert watchdog_of(document, job, step) is None, (
        "the step declares the variable, so its blank value is the one in "
        "force and the outer budgets are not consulted"
    )


@settings(deadline=None)
@given(spans=st.lists(SPANS, min_size=1, max_size=4))
def test_a_duration_is_the_sum_of_its_spans(spans: list[tuple[int, str]]) -> None:
    """Durations are parsed with humantime, which concatenates spans.

    Scenario: a period written as one or more count-and-unit spans, such
    as `1h 30min`.

    Invariant: it is worth the sum of its spans. An expression that
    matched a single span would refuse every concatenated duration, and
    one that matched only the short unit spellings would refuse `1hour`,
    both of which nextest accepts.
    """
    assert seconds(_span_text(spans)) == pytest.approx(float(_span_seconds(spans))), (
        "a humantime duration is the total of the spans written in it"
    )


@settings(deadline=None)
@given(spans=st.lists(SPANS, min_size=1, max_size=3), multiplier=MULTIPLIERS)
def test_the_per_test_budget_is_the_period_times_the_multiplier(
    spans: list[tuple[int, str]], multiplier: int
) -> None:
    """A per-test budget is a warning period counted out so many times.

    Scenario: a profile declaring a `slow-timeout` period and a
    `terminate-after` count.

    Invariant: the longest a test may run is their product. Every
    `terminate-after` in this repository is one, so a reading that
    ignored the multiplier entirely would give the same answer for every
    configuration here and a wrong one the day somebody raised it.
    """
    period = _span_text(spans)
    expected = float(_span_seconds(spans) * multiplier)
    assert largest_test_allowance(_profile(period, multiplier)) == pytest.approx(
        expected
    ), "the per-test budget is the warning period multiplied by the count"
