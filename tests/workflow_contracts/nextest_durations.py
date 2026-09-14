"""Nextest durations, and the faults a configuration can carry.

Split from ``nextest_budgets`` so both stay inside the 400-line limit
the lint gate enforces, and because these are the pieces the contract's
other readings share: a duration converter and the three faults a
configuration can carry.
"""

from __future__ import annotations

import re
import typing as typ

#: One span of a duration: an integer and a unit suffix, with optional
#: whitespace around each. nextest parses `slow-timeout.period` and
#: `grace-period` with humantime, whose durations are a concatenation of
#: such spans, so `"1h 30min 15s"` is one duration and not three.
_SPAN: typ.Final[re.Pattern[str]] = re.compile(
    r"\s*(?P<value>\d+)\s*(?P<unit>[^\W\d_]+)"
)

#: humantime's unit vocabulary, in seconds. Every spelling is listed
#: because humantime accepts all of them and a contract that knew only
#: the short forms would report a valid configuration as unreadable.
#: The case of `m` matters: `M` is a month, `m` is a minute. A month is
#: 30.44 days and a year 365.25 days, as humantime defines them.
_UNIT_SECONDS: typ.Final[dict[str, float]] = {
    "nsec": 1e-9,
    "ns": 1e-9,
    "usec": 1e-6,
    "us": 1e-6,
    # Escaped: U+00B5 MICRO SIGN, the spelling humantime accepts. Its
    # lookalike U+03BC GREEK SMALL LETTER MU is a different unit humantime
    # rejects, and the two are indistinguishable in a source file.
    "\u00b5s": 1e-6,
    "msec": 0.001,
    "ms": 0.001,
    "seconds": 1.0,
    "second": 1.0,
    "sec": 1.0,
    "s": 1.0,
    "minutes": 60.0,
    "minute": 60.0,
    "min": 60.0,
    "m": 60.0,
    "hours": 3600.0,
    "hour": 3600.0,
    "hrs": 3600.0,
    "hr": 3600.0,
    "h": 3600.0,
    "days": 86400.0,
    "day": 86400.0,
    "d": 86400.0,
    "weeks": 604800.0,
    "week": 604800.0,
    "wks": 604800.0,
    "wk": 604800.0,
    "w": 604800.0,
    "months": 2630016.0,
    "month": 2630016.0,
    "M": 2630016.0,
    "years": 31557600.0,
    "year": 31557600.0,
    "yrs": 31557600.0,
    "yr": 31557600.0,
    "y": 31557600.0,
}


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
    """Raised when the configuration cannot be read at all.

    Separate from a budget that bounds nothing. A file that is not TOML,
    a ``slow-timeout`` table with no ``period``, or a file declaring no
    ``slow-timeout`` anywhere, is a configuration this contract cannot
    reason about rather than one whose tiers are in the wrong order.
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
    >>> seconds("1h 30min")
    5400.0
    """
    total = 0.0
    consumed = 0
    for span in _SPAN.finditer(duration):
        if span.start() != consumed:
            break
        unit = _UNIT_SECONDS.get(span["unit"])
        if unit is None:
            break
        total += int(span["value"]) * unit
        consumed = span.end()
    if consumed == 0 or duration[consumed:].strip():
        message = f"unrecognized nextest duration {duration!r}"
        raise NextestConfigurationError(message, field="duration", value=duration)
    return total


def _is_period_count(multiplier: object) -> typ.TypeGuard[int]:
    """Return whether ``multiplier`` is a count of warning periods.

    ``bool`` is rejected before ``int`` on purpose. It is a subclass of
    ``int``, so ``terminate-after = true`` would otherwise read as one
    period and put a number on a tier the configuration never set.

    Parameters
    ----------
    multiplier : object
        The value the configuration declared.

    Returns
    -------
    bool
        Whether the value is a positive integer.
    """
    match multiplier:
        case bool():
            return False
        case int():
            return multiplier >= 1
        case _:
            return False


def periods(multiplier: object) -> int:
    """Return ``terminate-after`` as a count of warning periods.

    TOML returns whatever the document declared, and the budget is the
    period multiplied by this count, so a value that is not a count has
    to be refused here rather than converted. ``"3"`` is a string an
    author meant as a number and nextest will not read; ``true`` is a
    value that converts to a plausible budget and means nothing.

    Parameters
    ----------
    multiplier : object
        The value ``terminate-after`` declared.

    Returns
    -------
    int
        The count of warning periods.

    Raises
    ------
    NextestConfigurationError
        If the value is not a positive integer.

    Examples
    --------
    >>> periods(3)
    3
    """
    if _is_period_count(multiplier):
        return multiplier
    message = (
        f"terminate-after={multiplier!r} is not a positive whole number of "
        f"warning periods, so the per-test budget it scales cannot be read"
    )
    raise NextestConfigurationError(message, field="terminate-after", value=multiplier)
