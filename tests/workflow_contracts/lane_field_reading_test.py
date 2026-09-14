"""How the timeout contract narrows the fields it reads.

``timeout_reading_test.py`` covers the budgets the contract compares.
This module covers the layer beneath: turning one declared field into a
value, or refusing it. Those guards cannot be exercised against this
repository's own files, because every field here is already well formed.
A configuration writing ``1h 30min``, a ``terminate-after`` of ``true``,
or a ``timeout-minutes`` left as an unresolved expression is what they
exist for, and each is supplied here directly.
"""

from __future__ import annotations

import pytest
from lane_fields import CeilingValueError, condition, job_ceiling
from nextest_durations import NextestConfigurationError, periods, seconds


@pytest.mark.parametrize(
    ("duration", "expected"),
    [
        pytest.param("60s", 60.0, id="the-short-form"),
        pytest.param("2m", 120.0, id="minutes"),
        pytest.param("500ms", 0.5, id="milliseconds"),
        pytest.param("1hour", 3600.0, id="a-long-unit-name"),
        pytest.param("1h 30min 15s", 5415.0, id="three-compound-spans"),
        pytest.param("1h30m", 5400.0, id="compound-without-a-space"),
        pytest.param(" 90s ", 90.0, id="surrounding-whitespace"),
        pytest.param("1M", 2630016.0, id="a-month-is-not-a-minute"),
    ],
)
def test_a_humantime_duration_is_read_as_seconds(
    duration: str, expected: float
) -> None:
    """Durations are read the way nextest itself reads them, with humantime.

    humantime durations are a concatenation of spans, and every unit has
    several spellings. A reader that knew only ``ms``, ``s``, ``m`` and
    ``h`` would refuse a configuration nextest accepts and report the
    whole file as unreadable. The month case is the one that silently
    changes an answer rather than raising: ``M`` and ``m`` differ only in
    case and by a factor of forty-four thousand.
    """
    assert seconds(duration) == pytest.approx(expected), (
        f"{duration!r} is a humantime duration of {expected} s"
    )


@pytest.mark.parametrize(
    "duration",
    [
        pytest.param("", id="empty"),
        pytest.param("30", id="a-number-with-no-unit"),
        pytest.param("1.5h", id="a-fraction-humantime-rejects"),
        pytest.param("1x", id="an-unknown-unit"),
        pytest.param("1h 30xyz", id="a-second-span-with-an-unknown-unit"),
        pytest.param("1h junk", id="trailing-text"),
    ],
)
def test_a_duration_nextest_would_reject_is_refused(duration: str) -> None:
    """Reading a value nextest would not accept puts a false number on a tier.

    The trailing cases matter most: a reader that stopped at the first
    span it understood would take ``1h junk`` for an hour and report a
    tier the runner will never apply, because nextest refuses the file
    outright.
    """
    with pytest.raises(NextestConfigurationError, match=r"unrecognized"):
        seconds(duration)


def test_a_terminate_after_count_scales_the_period() -> None:
    """A positive whole number is the count of warning periods.

    It is the only shape nextest reads, and the per-test budget is the
    period multiplied by it.
    """
    assert periods(3) == 3, "a positive whole number is a count of periods"


@pytest.mark.parametrize(
    "multiplier",
    [
        pytest.param(True, id="a-boolean"),
        pytest.param("3", id="a-quoted-number"),
        pytest.param(2.5, id="a-fraction"),
        pytest.param(0, id="zero"),
        pytest.param(-1, id="negative"),
    ],
)
def test_a_terminate_after_that_is_not_a_count_is_refused(
    multiplier: object,
) -> None:
    """Converting these would put a plausible number on an absent tier.

    ``true`` is the one that has to be named explicitly: ``bool`` is a
    subclass of ``int``, so a plain numeric check would read it as one
    warning period and report a budget the configuration never set.
    ``"3"`` is an author's number nextest will not read at all.
    """
    with pytest.raises(NextestConfigurationError, match=r"terminate-after"):
        periods(multiplier)


@pytest.mark.parametrize(
    ("job", "expected"),
    [
        pytest.param({"timeout-minutes": 90}, 5400.0, id="an-unquoted-number"),
        pytest.param({"timeout-minutes": "120"}, 7200.0, id="a-quoted-number"),
        pytest.param({}, None, id="a-job-declaring-none"),
    ],
)
def test_a_job_ceiling_is_read_as_seconds(
    job: dict[str, object], expected: float | None
) -> None:
    """``timeout-minutes`` is minutes, and the arithmetic is in seconds.

    A job declaring none is not an error: it inherits GitHub's six-hour
    default, which the ordering contract judges separately.
    """
    assert job_ceiling(job) == pytest.approx(expected), (
        f"{job} must read as {expected!r} seconds"
    )


@pytest.mark.parametrize(
    "raw",
    [
        pytest.param("${{ env.SOMETHING }}", id="an-unresolved-expression"),
        pytest.param("", id="an-expression-that-resolved-to-nothing"),
        pytest.param(0, id="zero"),
        pytest.param(-5, id="negative"),
    ],
)
def test_a_job_ceiling_that_bounds_nothing_is_refused(raw: object) -> None:
    """Each of these leaves the job running to GitHub's own default.

    The job then looks bounded in the workflow and is not, which is the
    exact fault this pull request exists to correct one tier up. The
    failure names the key and the value so the message points at the text
    an author has to change.
    """
    with pytest.raises(CeilingValueError, match=r"timeout-minutes"):
        job_ceiling({"timeout-minutes": raw})


@pytest.mark.parametrize(
    ("raw", "expected"),
    [
        pytest.param(None, None, id="no-condition"),
        pytest.param(
            "github.event_name == 'push'",
            "github.event_name == 'push'",
            id="an-expression",
        ),
        pytest.param(False, "False", id="yaml-parsed-it-as-a-boolean"),
        pytest.param(0, "0", id="yaml-parsed-it-as-a-number"),
    ],
)
def test_a_condition_is_rendered_rather_than_refused(
    raw: object, expected: str | None
) -> None:
    """``if: false`` is a condition, not a malformed one.

    YAML decides the type, so a skipping condition can arrive as a
    boolean or a number. Refusing those would hide the lane the ordering
    contract most needs to report, so each is rendered and the pinned
    comparison then names the spelling found.
    """
    assert condition(raw) == expected, (
        f"{raw!r} must read as the condition {expected!r}"
    )
