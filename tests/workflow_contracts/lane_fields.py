"""Narrows the fields a coverage lane declares, once, at one boundary.

The YAML loader hands back values it has not judged, so a field read
straight out of a parsed document is text, a number, a boolean or an
unresolved expression, and nothing says which. Every such field is
narrowed here and nowhere else: :mod:`coverage_lanes` assembles lanes
out of values this module has already refused or accepted, and nothing
downstream sees an unvalidated one.

Split out of :mod:`coverage_lanes` when that module crossed the
400-line limit the Python lint gate enforces. The seam is the one its
docstring already claimed.
"""

from __future__ import annotations

import typing as typ

from timeout_budgets import WATCHDOG_VARIABLE

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: One parsed YAML mapping, before any field of it has been read. The
#: values are ``object`` rather than ``typ.Any`` so a field cannot be
#: used without being narrowed first.
type Node = cabc.Mapping[str, object]


class LaneValueError(ValueError):
    """Raised when a bound a lane declares cannot be read as one.

    The base of the two faults below so ``_coverage_job`` can add the
    lane's coordinate to either without naming each in turn.
    """


class WatchdogValueError(LaneValueError):
    """Raised when a workflow's watchdog value cannot be read as seconds.

    Distinguished from an unset watchdog rather than folded into it. A
    lane that sets nothing inherits the action's default, which is one
    fault; a lane that sets ``abc`` has an author who meant something
    and got neither, which is another. Reporting the second as the first
    would name the wrong remedy.
    """


class CeilingValueError(LaneValueError):
    """Raised when a job's ``timeout-minutes`` cannot be read as one.

    Kept apart from the watchdog fault because the remedy differs: a
    ceiling is a job key GitHub itself enforces, so a job carrying an
    unreadable one runs to GitHub's six-hour default rather than to the
    bound its author wrote.
    """


def condition(raw: object) -> str | None:
    """Return a step's or job's ``if`` as the text GitHub evaluates.

    A condition is an expression, but YAML decides its type: ``if:
    false`` arrives as a boolean and ``if: 0`` as an integer. Both are
    conditions that skip the lane, so they are rendered rather than
    refused, and the pinned comparison then reports the spelling found.

    Parameters
    ----------
    raw : object
        The value the workflow set, as the YAML parser returned it.

    Returns
    -------
    str or None
        The condition's text, or None when the level sets none.
    """
    match raw:
        case None:
            return None
        case str():
            return raw
        case _:
            return str(raw)


def mapping_of(value: object) -> Node | None:
    """Return the value as a mapping of string keys, or None.

    Parameters
    ----------
    value : object
        A value the YAML loader produced.

    Returns
    -------
    Node or None
        The mapping, or None when the value is not one.
    """
    match value:
        case dict():
            return {str(key): item for key, item in value.items()}
        case _:
            return None


def mappings_in(container: object) -> list[Node]:
    """Return the mappings in a parsed sequence, ignoring anything else.

    Parameters
    ----------
    container : object
        The parsed value, which need not be a list.

    Returns
    -------
    list[Node]
        The mappings, in order.
    """
    match container:
        case list():
            return [
                mapping
                for item in container
                if (mapping := mapping_of(item)) is not None
            ]
        case _:
            return []


def budget_from(raw: object) -> float | None:
    """Return one source's watchdog budget, or None when it sets none.

    A blank or whitespace-only value is a source that says nothing, so
    it falls through to the next one. That is what a workflow writes
    when it interpolates an expression that resolved to nothing.

    Anything else that is not a positive number of seconds is refused
    with the value in the message. The shared action reads a
    non-positive value as no timeout at all, so a lane carrying one has
    no third tier while appearing to declare one.

    Parameters
    ----------
    raw : object
        The value the workflow set, as the YAML parser returned it.

    Returns
    -------
    float or None
        The budget in seconds, or None when the source sets none.

    Raises
    ------
    WatchdogValueError
        If the value is present and non-blank but not a positive number
        of seconds.
    """
    if raw is None:
        return None
    text = str(raw).strip()
    if not text:
        return None
    try:
        budget = float(text)
    except ValueError as error:
        message = (
            f"{WATCHDOG_VARIABLE}={raw!r} is not a number of seconds; the "
            f"lane sets a watchdog its author meant and the action will not "
            f"read"
        )
        raise WatchdogValueError(message) from error
    if budget <= 0:
        message = (
            f"{WATCHDOG_VARIABLE}={raw!r} is not positive, so the cargo "
            f"invocation is unbounded while appearing to be bounded"
        )
        raise WatchdogValueError(message)
    return budget


def job_ceiling(job: Node) -> float | None:
    """Return the job's ``timeout-minutes`` in seconds, or None.

    Parameters
    ----------
    job : Node
        The parsed job.

    Returns
    -------
    float or None
        The ceiling in seconds, or None when the job declares none.

    Raises
    ------
    CeilingValueError
        If the value is present but not a positive number of minutes.
        An unresolved expression such as ``${{ env.SOMETHING }}`` and a
        zero or negative ceiling both leave the job unbounded while
        appearing to declare a bound.
    """
    raw = job.get("timeout-minutes")
    if raw is None:
        return None
    try:
        minutes = float(str(raw).strip())
    except ValueError as error:
        message = (
            f"timeout-minutes={raw!r} is not a number of minutes; the job "
            f"declares a ceiling the arithmetic cannot check"
        )
        raise CeilingValueError(message) from error
    if minutes <= 0:
        message = (
            f"timeout-minutes={raw!r} is not positive, so the job runs to "
            f"GitHub's own default while appearing to be bounded"
        )
        raise CeilingValueError(message)
    return minutes * 60.0
