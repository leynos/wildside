"""How the timeout contract reads the files it compares.

Every assertion in ``timeout_ordering_test.py`` rests on turning three
files into comparable seconds. Those readings can be wrong while no file
is wrong, and this repository's own configuration cannot expose most of
the ways they can be: every ``terminate-after`` here is one, so a reader
that ignored the multiplier entirely would give the same answer, and no
lane sets a malformed watchdog. So the readings are driven with
controlled values here.
"""

from __future__ import annotations

import pytest
from coverage_lanes import WatchdogValueError, coverage_jobs_of, watchdog_of
from timeout_budgets import (
    CEILING_MARGIN_SECONDS,
    UnboundedTestError,
    global_timeout,
    grace_period,
    largest_test_allowance,
    required_ceiling,
    termination_allowance,
)


@pytest.mark.parametrize(
    ("config_text", "expected"),
    [
        pytest.param(
            'slow-timeout = { period = "180s", terminate-after = 1 }',
            180.0,
            id="a-single-period",
        ),
        pytest.param(
            'slow-timeout = { period = "60s", terminate-after = 5 }',
            300.0,
            id="five-warning-periods",
        ),
        pytest.param(
            'slow-timeout = { period = "2m", terminate-after = 3 }',
            360.0,
            id="minutes-times-three",
        ),
        pytest.param(
            'slow-timeout = { period = "30s", terminate-after = 2, '
            'grace-period = "5s" }\n'
            'slow-timeout = { period = "60s", terminate-after = 1 }',
            60.0,
            id="the-largest-of-several",
        ),
    ],
)
def test_the_largest_per_test_allowance_counts_the_multiplier(
    config_text: str, expected: float
) -> None:
    """``terminate-after`` scales the period; the budget is their product.

    This is the reading every comparison in the contract rests on, and
    it is the one easy to get wrong. Every multiplier in this
    repository's own configuration is one, so against that file a reader
    that ignored the multiplier would give the same answer and prove
    nothing.
    """
    assert largest_test_allowance(config_text) == pytest.approx(expected), (
        f"{config_text!r} must yield a {expected:.0f}s largest per-test "
        f"allowance; terminate-after scales the period"
    )


@pytest.mark.parametrize(
    "config_text",
    [
        pytest.param('slow-timeout = "2m"', id="a-bare-duration"),
        pytest.param(
            'slow-timeout = { period = "2m" }',
            id="a-table-without-terminate-after",
        ),
        pytest.param(
            'slow-timeout = { period = "2m", grace-period = "5s" }',
            id="a-table-with-only-a-grace-period",
        ),
    ],
)
def test_a_slow_timeout_that_never_terminates_is_refused(config_text: str) -> None:
    """``terminate-after`` is optional, and without it nothing is bounded.

    nextest marks the test slow, warns once per period, and lets it run
    on. Reading such a configuration as a period-long budget would put a
    number on the tier that is missing, and every comparison above it
    would then pass against a tier that does not exist.
    """
    with pytest.raises(UnboundedTestError, match=r"terminate-after"):
        largest_test_allowance(config_text)


def test_a_grace_period_is_not_read_as_a_per_test_budget() -> None:
    """The two keys sit in the same inline table.

    A matcher reading `period` as a substring would take a grace period
    for a per-test budget whenever the former were the larger, which
    would silently raise the whole-run budget this contract demands.
    """
    config_text = (
        'slow-timeout = { period = "30s", terminate-after = 1, grace-period = "30m" }'
    )
    assert largest_test_allowance(config_text) == pytest.approx(30.0), (
        "the per-test ceiling read a grace period as a slow-timeout"
    )


def test_the_termination_allowance_adds_its_two_terms() -> None:
    """The grace period and the teardown margin are added, not maximised.

    A single floor over the two would absorb every grace period below
    the margin, so raising one would look free until the run it
    cancelled. nextest's own default applies when none is named.
    """
    assert termination_allowance('grace-period = "5s"') == pytest.approx(65.0), (
        "a five-second grace period plus the sixty-second margin is 65 s"
    )
    assert termination_allowance('grace-period = "5m"') == pytest.approx(360.0), (
        "raising the grace period must raise the allowance with it"
    )
    assert termination_allowance("") == pytest.approx(grace_period("") + 60.0), (
        "with no grace period named, nextest's default is the first term"
    )


def test_the_whole_run_budget_is_read_only_at_the_root() -> None:
    """``global-timeout`` is a profile key, not a per-override one.

    Reading it from anywhere in the file would let an indented value in
    a table be taken for the whole-run budget.
    """
    assert global_timeout('global-timeout = "60m"') == pytest.approx(3600.0)
    assert global_timeout('  global-timeout = "5m"') is None, (
        "an indented global-timeout is not the profile's whole-run budget"
    )
    assert global_timeout('slow-timeout = { period = "60s" }') is None


def test_the_required_ceiling_carries_all_three_terms() -> None:
    """Watchdogs, measured work outside them, and the margin.

    Both ceilings in this repository clear the smaller requirement too,
    so dropping the margin changes nothing the assertion over the
    workflows can see. Driving the derivation with controlled numbers is
    what makes the missing term visible.
    """
    assert required_ceiling([5400.0, 1800.0], 900.0) == pytest.approx(
        7200.0 + 900.0 + CEILING_MARGIN_SECONDS
    ), "two watchdogs, the allowance and the margin are all added"
    assert required_ceiling([5400.0], 0.0) == pytest.approx(
        5400.0 + CEILING_MARGIN_SECONDS
    ), "the margin applies even when nothing runs outside the watchdog"
    assert required_ceiling([], 0.0) == pytest.approx(CEILING_MARGIN_SECONDS), (
        "the margin is a term of its own, not a fraction of the others"
    )


def _lane(
    watchdog: object,
) -> tuple[dict[str, object], dict[str, object], dict[str, object]]:
    """Return a document, job and step setting one watchdog value.

    Parameters
    ----------
    watchdog : object
        The value to put in the step's ``env``.

    Returns
    -------
    tuple
        Document, job and step, ready for ``watchdog_of``.
    """
    return ({}, {}, {"env": {"RUN_RUST_CARGO_WAIT_TIMEOUT": watchdog}})


@pytest.mark.parametrize(
    ("value", "expected"),
    [
        pytest.param("5400", 5400.0, id="a-quoted-number"),
        pytest.param(5400, 5400.0, id="an-unquoted-number"),
        pytest.param(" 5400 ", 5400.0, id="surrounding-whitespace"),
        pytest.param("", None, id="an-expression-that-resolved-to-nothing"),
        pytest.param("   ", None, id="whitespace-only"),
    ],
)
def test_a_watchdog_value_is_read_or_falls_through(
    value: object, expected: float | None
) -> None:
    """A blank value says nothing, so the next level decides.

    That is what a workflow writes when it interpolates an expression
    that resolved to nothing, and treating it as a budget of zero would
    report an unbounded lane as the tightest one in the estate.
    """
    document, job, step = _lane(value)
    assert watchdog_of(document, job, step) == expected


@pytest.mark.parametrize(
    "value",
    [
        pytest.param("abc", id="not-a-number"),
        pytest.param("0", id="zero"),
        pytest.param("-1", id="negative"),
    ],
)
def test_a_watchdog_value_the_action_cannot_use_is_refused(value: str) -> None:
    """The action reads a non-positive value as no timeout at all.

    A lane carrying one has no third tier while appearing to declare
    one, which is worse than declaring none: the guide and this contract
    would both record a watchdog that never fires.
    """
    document, job, step = _lane(value)
    with pytest.raises(WatchdogValueError, match=r"RUN_RUST_CARGO_WAIT_TIMEOUT"):
        watchdog_of(document, job, step)


def test_the_watchdog_is_resolved_innermost_first() -> None:
    """Step, then job, then workflow, as GitHub resolves them.

    Both workflows here set the value at job level, so a reader that
    consulted only the step would find nothing and report every lane as
    inheriting the action's default, which is exactly backwards.
    """
    document = {"env": {"RUN_RUST_CARGO_WAIT_TIMEOUT": "100"}}
    job = {"env": {"RUN_RUST_CARGO_WAIT_TIMEOUT": "200"}}
    step = {"env": {"RUN_RUST_CARGO_WAIT_TIMEOUT": "300"}}
    assert watchdog_of(document, job, step) == pytest.approx(300.0)
    assert watchdog_of(document, job, {}) == pytest.approx(200.0)
    assert watchdog_of(document, {}, {}) == pytest.approx(100.0)
    assert watchdog_of({}, {}, {}) is None


def test_a_synthetic_workflow_is_read_as_one_lane_per_job() -> None:
    """The lane reading is driven with documents rather than the tree.

    A job running the coverage action twice is the case the ceiling
    arithmetic exists for, and no workflow here has one, so the reading
    that counts steps has to be exercised against a document written for
    it.
    """
    step = {
        "uses": ("leynos/shared-actions/.github/actions/generate-coverage@abc123"),
        "env": {"RUN_RUST_CARGO_WAIT_TIMEOUT": "1800"},
    }
    documents = {
        "synthetic.yml": {
            "jobs": {
                "twice": {"timeout-minutes": 90, "steps": [step, step]},
                "none": {"steps": [{"run": "cargo test"}]},
            }
        }
    }
    jobs = coverage_jobs_of(documents)
    assert [(job.workflow, job.job, job.steps) for job in jobs] == [
        ("synthetic.yml", "twice", 2)
    ]
    assert jobs[0].watchdogs == (1800.0, 1800.0)
    assert jobs[0].job_timeout == pytest.approx(5400.0)
