"""Reading `.config/nextest.toml` as the runner reads it.

Separated from ``timeout_budgets`` so the configuration reading and the
values it is compared against stay legible apart, and so neither module
outgrows the 400-line limit the lint gate enforces.

The configuration is parsed with ``tomllib`` rather than matched as
text. A text match finds a key inside a comment, inside a ``filter``
string, or in a table nextest never consults, and reports a budget the
runner does not use. The commented-out ``global-timeout`` is the case
that matters most, because the contract requires that tier to be
present.
"""

from __future__ import annotations

import tomllib
import typing as typ
from itertools import starmap

from nextest_durations import (
    NextestConfigurationError,
    UnboundedTestError,
    seconds,
)
from timeout_budgets import (
    NEXTEST_DEFAULT_GRACE_PERIOD_SECONDS,
    TERMINATION_SAFETY_MARGIN_SECONDS,
)


def _table(value: object) -> dict[str, object]:
    """Return a parsed value as a table, or an empty one.

    ``tomllib`` returns whatever the document said, so a configuration
    naming a scalar where a table belongs yields nothing here rather
    than raising several frames away.

    Parameters
    ----------
    value : object
        Any value ``tomllib`` produced.

    Returns
    -------
    dict[str, object]
        The table, or an empty one when the value is not a table.
    """
    if not isinstance(value, dict):
        return {}
    return {str(key): item for key, item in value.items()}


def _parsed(config_text: str) -> dict[str, object]:
    """Return the nextest configuration as TOML.

    Parameters
    ----------
    config_text : str
        The nextest configuration file's text.

    Returns
    -------
    dict[str, object]
        The parsed document.

    Raises
    ------
    NextestConfigurationError
        If the text is not valid TOML.
    """
    try:
        return tomllib.loads(config_text)
    except tomllib.TOMLDecodeError as error:
        message = f"the nextest configuration is not valid TOML: {error}"
        raise NextestConfigurationError(
            message, field="config", value=config_text
        ) from error


def _budget_tables(config_text: str) -> list[tuple[str, dict[str, object]]]:
    """Return every table nextest reads a per-test budget from.

    Each profile's own table and each of its ``[[overrides]]`` entries,
    with the dotted path that names it so a failure can say which one is
    at fault.

    Parameters
    ----------
    config_text : str
        The nextest configuration file's text.

    Returns
    -------
    list of tuple
        The dotted path and the table, in file order.
    """
    tables: list[tuple[str, dict[str, object]]] = []
    for name, raw in _table(_parsed(config_text).get("profile")).items():
        profile = _table(raw)
        tables.append((f"profile.{name}", profile))
        overrides = profile.get("overrides")
        entries = overrides if isinstance(overrides, list) else []
        tables.extend(
            (f"profile.{name}.overrides[{index}]", _table(entry))
            for index, entry in enumerate(entries)
        )
    return tables


def _slow_timeouts(config_text: str) -> list[tuple[str, object]]:
    """Return every ``slow-timeout`` the configuration declares.

    Parameters
    ----------
    config_text : str
        The nextest configuration file's text.

    Returns
    -------
    list of tuple
        The dotted path of the declaring table and the value.
    """
    return [
        (path, table["slow-timeout"])
        for path, table in _budget_tables(config_text)
        if "slow-timeout" in table
    ]


def _table_budget(path: str, table: dict[str, object]) -> float:
    """Return one inline ``slow-timeout`` table's per-test budget.

    Parameters
    ----------
    path : str
        The dotted path of the declaring table, for the message.
    table : dict[str, object]
        The parsed table.

    Returns
    -------
    float
        The period multiplied by ``terminate-after``, in seconds.

    Raises
    ------
    NextestConfigurationError
        If the table names no ``period``.
    UnboundedTestError
        If the table names no ``terminate-after``, so nextest marks the
        test slow and lets it run on.
    """
    period = table.get("period")
    if not isinstance(period, str):
        message = f"{path}.slow-timeout names no period: {table!r}"
        raise NextestConfigurationError(message, field="period", value=table)
    multiplier = table.get("terminate-after")
    if multiplier is None:
        message = (
            f"{path}.slow-timeout sets no terminate-after, so nextest marks "
            f"the test slow and lets it run on; there is no per-test tier to "
            f"compare against"
        )
        raise UnboundedTestError(message, field="terminate-after", value=table)
    return seconds(period) * float(str(multiplier))


def _bare_budget(path: str, period: str) -> typ.NoReturn:
    """Refuse a ``slow-timeout`` written as a bare duration.

    Parameters
    ----------
    path : str
        The dotted path of the declaring table, for the message.
    period : str
        The duration the configuration named.

    Raises
    ------
    UnboundedTestError
        Always. The bare form sets a warning period with no
        ``terminate-after``, so no test is ever terminated by it.
    """
    message = (
        f'{path}.slow-timeout = "{period}" sets a warning period with no '
        f"terminate-after, so nextest reports the test as slow and never "
        f"stops it; there is no per-test tier to compare against"
    )
    raise UnboundedTestError(message, field="slow-timeout", value=period)


def _budget_of(path: str, value: object) -> float:
    """Return the per-test budget one ``slow-timeout`` declares.

    Parameters
    ----------
    path : str
        The dotted path of the declaring table, for the message.
    value : object
        The parsed value, a table or a bare duration.

    Returns
    -------
    float
        The budget in seconds.

    Raises
    ------
    NextestConfigurationError
        If the value is neither a table nor a duration.
    """
    match value:
        case str():
            return _bare_budget(path, value)
        case dict():
            return _table_budget(path, _table(value))
        case _:
            message = f"{path}.slow-timeout is neither a table nor a duration"
            raise NextestConfigurationError(message, field="slow-timeout", value=value)


def largest_test_allowance(config_text: str) -> float:
    r"""Return the longest a single test may run, in seconds.

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
        If the configuration declares no ``slow-timeout`` at all. A
        ``slow-timeout`` that terminates nothing raises
        :class:`UnboundedTestError` from :func:`_budget_of`.

    Examples
    --------
    >>> largest_test_allowance(
    ...     '[profile.default]\n'
    ...     'slow-timeout = { period = "60s", terminate-after = 5 }\n'
    ... )
    300.0
    """
    budgets = list(starmap(_budget_of, _slow_timeouts(config_text)))
    if not budgets:
        message = (
            "the nextest configuration declares no slow-timeout, so no test "
            "is bounded and there is no per-test tier to compare against"
        )
        raise NextestConfigurationError(
            message, field="slow-timeout", value=config_text
        )
    return max(budgets)


def bounds_a_single_test(config_text: str, profile: str = "default") -> bool:
    """Return whether a profile's own table terminates a slow test.

    Only the profile's own ``slow-timeout`` counts. An override bounds
    the tests its filter matches; the profile's own bounds the rest, so
    a profile whose only ``terminate-after`` sits in an override leaves
    every unmatched test running with no bound at all while
    :func:`largest_test_allowance` still reports a comfortable number.

    Parameters
    ----------
    config_text : str
        The nextest configuration file's text.
    profile : str
        The profile to read.

    Returns
    -------
    bool
        True when that profile's own ``slow-timeout`` is a table setting
        ``terminate-after``.
    """
    own = _table(_table(_parsed(config_text).get("profile")).get(profile))
    table = own.get("slow-timeout")
    return isinstance(table, dict) and table.get("terminate-after") is not None


def grace_period(config_text: str) -> float:
    r"""Return the longest grace period the configuration names, in seconds.

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
    >>> grace_period(
    ...     '[profile.default]\n'
    ...     'slow-timeout = { period = "60s", terminate-after = 1, '
    ...     'grace-period = "5s" }\n'
    ... )
    5.0
    """
    periods = [
        seconds(grace)
        for _, value in _slow_timeouts(config_text)
        if isinstance(value, dict)
        and isinstance(grace := value.get("grace-period"), str)
    ]
    return max(periods, default=NEXTEST_DEFAULT_GRACE_PERIOD_SECONDS)


def termination_allowance(config_text: str) -> float:
    r"""Return the time nextest may take to stop the run, in seconds.

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
    >>> termination_allowance(
    ...     '[profile.default]\n'
    ...     'slow-timeout = { period = "60s", terminate-after = 1, '
    ...     'grace-period = "5s" }\n'
    ... )
    65.0
    """
    return grace_period(config_text) + TERMINATION_SAFETY_MARGIN_SECONDS


def global_timeout(config_text: str) -> float | None:
    r"""Return the whole-run budget, or None when none is set.

    Read from ``[profile.default]`` alone. nextest's other profiles
    inherit that table unless they override it, and an ``[[overrides]]``
    entry cannot carry one.

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
    >>> global_timeout('[profile.default]\nglobal-timeout = "60m"\n')
    3600.0
    """
    profile = _table(_table(_parsed(config_text).get("profile")).get("default"))
    budget = profile.get("global-timeout")
    return seconds(budget) if isinstance(budget, str) else None
