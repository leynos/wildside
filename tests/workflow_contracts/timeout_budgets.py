"""Reading the timers that can end a test run.

The contract in :mod:`timeout_ordering_test` compares budgets written
down in three different files. Turning those files into comparable
seconds is the part that can be wrong without any file being wrong, so
it lives here where it can be read on its own.

The reading that matters most is the per-test one. nextest warns once
per ``period`` and terminates after ``terminate-after`` of them, so the
budget is their product, and a ``slow-timeout`` that names no
``terminate-after`` never terminates anything at all. That form is
refused rather than read as a single period: a test under it runs until
some outer tier stops it, which is the absence of tier one rather than a
small tier one.

See "Test timeouts: four tiers, outermost last" in
``docs/developers-guide.md``, and the canonical wording in
`leynos/shared-actions`' `generate-coverage` README.
"""

from __future__ import annotations

import re
import typing as typ
from pathlib import Path

if typ.TYPE_CHECKING:
    import collections.abc as cabc

REPO_ROOT: typ.Final[Path] = Path(__file__).resolve().parents[2]
WORKFLOWS_DIRECTORY: typ.Final[Path] = REPO_ROOT / ".github" / "workflows"
NEXTEST_CONFIG: typ.Final[Path] = REPO_ROOT / ".config" / "nextest.toml"

#: The environment variable the shared coverage action reads for its
#: wall-clock cap on one `cargo` invocation.
WATCHDOG_VARIABLE: typ.Final[str] = "RUN_RUST_CARGO_WAIT_TIMEOUT"

#: The action whose steps run under that watchdog.
COVERAGE_ACTION: typ.Final[str] = (
    "leynos/shared-actions/.github/actions/generate-coverage"
)

#: Everything in a coverage job that is not a `cargo` invocation the
#: watchdog bounds: checkout, toolchain setup, the database fixtures, and
#: above all the cache save and restore. The job timer covers it; the
#: watchdog does not.
#:
#: Measured from the worst of several runs rather than one. Across ten
#: successful `coverage-main.yml` runs the widest gap between the
#: coverage step and its job was 522 s on run 33921794186, where the
#: database fixtures and the artefact upload run outside the coverage
#: step. Across ten of `ci.yml` it was 59 s on run 33938872167. Fifteen
#: minutes covers the worse of those, and none of those runs was
#: genuinely cold.
OUTSIDE_WATCHDOG_ALLOWANCE_SECONDS: typ.Final[float] = 15 * 60.0

#: How far a ceiling must sit above the sum it contains, rather than
#: merely reaching it. A ceiling equal to that sum cancels the job at
#: the moment the watchdog would have reported the overrun, and the
#: report is the only thing that makes an overrun actionable.
CEILING_MARGIN_SECONDS: typ.Final[float] = 15 * 60.0

#: What nextest allows a test between `SIGTERM` and `SIGKILL` when the
#: configuration names no `grace-period`.
NEXTEST_DEFAULT_GRACE_PERIOD_SECONDS: typ.Final[float] = 10.0

#: Added to that grace period to cover the teardown and report writing
#: that follow it. A separate term rather than a floor over the two, so
#: raising a grace period raises the requirement instead of vanishing
#: into it.
TERMINATION_SAFETY_MARGIN_SECONDS: typ.Final[float] = 60.0

#: Build time inside a `cargo` invocation before nextest starts its own
#: clock. Only used if a `global-timeout` appears.
COLD_BUILD_ALLOWANCE_SECONDS: typ.Final[float] = 10 * 60.0

_DURATION: typ.Final[re.Pattern[str]] = re.compile(
    r"^\s*(?P<value>\d+(?:\.\d+)?)\s*(?P<unit>ms|s|m|h)\s*$"
)

_UNIT_SECONDS: typ.Final[dict[str, float]] = {
    "ms": 0.001,
    "s": 1.0,
    "m": 60.0,
    "h": 3600.0,
}

#: One `slow-timeout` inline table, captured whole so the period and the
#: multiplier that scales it are read together. Reading `period` alone
#: would understate the budget by whatever `terminate-after` says.
_SLOW_TIMEOUT: typ.Final[re.Pattern[str]] = re.compile(
    r"slow-timeout\s*=\s*\{(?P<body>[^}]*)\}"
)

#: The other spelling: `slow-timeout = "60s"`. It sets a warning period
#: and nothing else, so it bounds no test.
_BARE_SLOW_TIMEOUT: typ.Final[re.Pattern[str]] = re.compile(
    r'slow-timeout\s*=\s*"(?P<period>[^"]+)"'
)

_PERIOD: typ.Final[re.Pattern[str]] = re.compile(r'(?<!-)period\s*=\s*"([^"]+)"')
_TERMINATE_AFTER: typ.Final[re.Pattern[str]] = re.compile(
    r"terminate-after\s*=\s*(\d+)"
)
_GRACE_PERIOD: typ.Final[re.Pattern[str]] = re.compile(r'grace-period\s*=\s*"([^"]+)"')


class TimeoutBudgetError(ValueError):
    """Raised when a configured budget cannot be read as a bound.

    Attributes
    ----------
    field : str
        The configuration key at fault.
    value : object
        What the configuration said, so the message names the text an
        author has to change rather than only the rule it broke.
    """

    def __init__(self, message: str, *, field: str, value: object) -> None:
        """Record the failing key and its value alongside the message.

        Parameters
        ----------
        message : str
            The human-readable explanation.
        field : str
            The configuration key at fault.
        value : object
            What the configuration said.
        """
        super().__init__(message)
        self.field = field
        self.value = value


class NextestConfigurationError(TimeoutBudgetError):
    """Raised when the nextest configuration cannot be read at all.

    Separate from a budget that reads as unbounded. A ``slow-timeout``
    with no ``period``, or a file with no ``slow-timeout`` anywhere, is
    a configuration this contract cannot reason about rather than one
    whose tiers are in the wrong order.
    """


class UnboundedTestError(TimeoutBudgetError):
    """Raised when a ``slow-timeout`` terminates no test.

    Distinguished from a malformed value. ``terminate-after`` is
    optional, and without it nextest marks a test slow and lets it run
    on, so the configuration parses, reads as deliberate, and bounds
    nothing. Reporting that as a period-long budget would put a number
    on the tier that is missing.
    """


def seconds(duration: str) -> float:
    """Convert a nextest duration to seconds.

    Parameters
    ----------
    duration : str
        A duration as nextest spells it, such as ``"120s"``.

    Returns
    -------
    float
        The duration in seconds.

    Raises
    ------
    NextestConfigurationError
        If the text is not a duration nextest would accept.

    Examples
    --------
    >>> seconds("2m")
    120.0
    """
    match = _DURATION.match(duration)
    if match is None:
        message = f"unrecognized nextest duration {duration!r}"
        raise NextestConfigurationError(message, field="duration", value=duration)
    return float(match["value"]) * _UNIT_SECONDS[match["unit"]]


def _table_budget(body: str) -> float:
    """Return one inline ``slow-timeout`` table's per-test budget.

    Parameters
    ----------
    body : str
        The text between the table's braces.

    Returns
    -------
    float
        The period multiplied by ``terminate-after``, in seconds.

    Raises
    ------
    NextestConfigurationError
        If the table names no ``period``.
    UnboundedTestError
        If the table names no ``terminate-after``, so nextest warns
        about a slow test forever and never stops it.
    """
    period = _PERIOD.search(body)
    if period is None:
        message = f"slow-timeout without a period: {body!r}"
        raise NextestConfigurationError(message, field="period", value=body)
    terminate = _TERMINATE_AFTER.search(body)
    if terminate is None:
        message = (
            f"slow-timeout {{{body.strip()}}} sets no terminate-after, so "
            f"nextest marks the test slow and lets it run on; there is no "
            f"per-test tier to compare against"
        )
        raise UnboundedTestError(message, field="terminate-after", value=body)
    return seconds(period[1]) * int(terminate[1])


def _bare_budget(period: str) -> typ.NoReturn:
    """Refuse a ``slow-timeout`` written as a bare duration.

    Parameters
    ----------
    period : str
        The duration the configuration named.

    Raises
    ------
    UnboundedTestError
        Always. The bare form sets a warning period with no
        ``terminate-after``, so no test is ever terminated by it.
    """
    message = (
        f'slow-timeout = "{period}" sets a warning period with no '
        f"terminate-after, so nextest reports the test as slow and never "
        f"stops it; there is no per-test tier to compare against"
    )
    raise UnboundedTestError(message, field="slow-timeout", value=period)


def largest_test_allowance(config_text: str) -> float:
    """Return the longest a single test may run, in seconds.

    Parameters
    ----------
    config_text : str
        The nextest configuration file's text.

    Returns
    -------
    float
        The longest per-test budget, period multiplied by
        ``terminate-after``.

    Raises
    ------
    NextestConfigurationError
        If the configuration sets no ``slow-timeout`` at all.
    UnboundedTestError
        If any ``slow-timeout`` terminates no test.

    Examples
    --------
    >>> largest_test_allowance(
    ...     'slow-timeout = { period = "60s", terminate-after = 5 }'
    ... )
    300.0
    """
    budgets = [
        _table_budget(match["body"]) for match in _SLOW_TIMEOUT.finditer(config_text)
    ]
    remainder = _SLOW_TIMEOUT.sub("", config_text)
    for bare in _BARE_SLOW_TIMEOUT.finditer(remainder):
        _bare_budget(bare["period"])
    if not budgets:
        message = (
            "the nextest configuration sets no slow-timeout, so no test is "
            "bounded and there is no per-test tier to compare against"
        )
        raise NextestConfigurationError(
            message, field="slow-timeout", value=config_text
        )
    return max(budgets)


def grace_period(config_text: str) -> float:
    """Return the longest grace period the configuration names, in seconds.

    Parameters
    ----------
    config_text : str
        The nextest configuration file's text.

    Returns
    -------
    float
        The largest configured grace period, or nextest's default when
        the configuration names none.

    Examples
    --------
    >>> grace_period('grace-period = "5s"')
    5.0
    """
    periods = _GRACE_PERIOD.findall(config_text)
    return max(
        (seconds(period) for period in periods),
        default=NEXTEST_DEFAULT_GRACE_PERIOD_SECONDS,
    )


def termination_allowance(config_text: str) -> float:
    """Return the time nextest may take to stop the run, in seconds.

    Two terms, not one. Hitting the whole-run budget starts nextest's
    ordinary termination procedure rather than stopping the run: on Unix
    it signals the process group and waits ``slow-timeout.grace-period``
    before killing it. That grace period is the first term; the second
    is a fixed margin for the teardown and report writing that follow
    it. A single floor over the two would absorb every grace period
    below the margin, so raising one would look free until the run it
    cancelled.

    Parameters
    ----------
    config_text : str
        The nextest configuration file's text.

    Returns
    -------
    float
        The grace period plus the safety margin.

    Examples
    --------
    >>> termination_allowance('grace-period = "5s"')
    65.0
    """
    return grace_period(config_text) + TERMINATION_SAFETY_MARGIN_SECONDS


def global_timeout(config_text: str) -> float | None:
    """Return the whole-run budget, or None when none is set.

    Parameters
    ----------
    config_text : str
        The nextest configuration file's text.

    Returns
    -------
    float or None
        The whole-run budget in seconds, or None.

    Examples
    --------
    >>> global_timeout('global-timeout = "60m"')
    3600.0
    """
    match = re.search(r'^global-timeout\s*=\s*"([^"]+)"', config_text, re.MULTILINE)
    return None if match is None else seconds(match[1])


def required_ceiling(budgets: cabc.Sequence[float], allowance: float) -> float:
    """Return the smallest acceptable ceiling for one job, in seconds.

    Three terms. Each coverage step may legitimately spend its whole
    watchdog, so the sum is the floor. The measured work outside those
    windows is added because the job timer covers it and the watchdogs
    do not. The margin is added because a ceiling equal to that sum
    cancels the job at the moment the watchdog would have reported the
    overrun.

    Parameters
    ----------
    budgets : cabc.Sequence[float]
        One watchdog budget per coverage step in the job.
    allowance : float
        The measured work outside those windows, in seconds.

    Returns
    -------
    float
        The smallest acceptable ceiling, in seconds.

    Examples
    --------
    One 5,400 s watchdog with 900 s of work outside it needs a ceiling
    of 7,200 s once the 900 s margin is added, which is this
    repository's 120 minutes:

    >>> required_ceiling([5400.0], 900.0)
    7200.0

    A job running the action twice must contain both budgets:

    >>> required_ceiling([5400.0, 1800.0], 0.0)
    8100.0
    """
    return sum(budgets) + allowance + CEILING_MARGIN_SECONDS
