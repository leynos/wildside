"""Reads the runner labels a job's ``runs-on`` can select.

Every placement contract asks which labels a job can run on: whether a
paid lane falls back for a fork, whether a label is registered with
actionlint, whether a scheduled job stays hosted. They share this one
reader so that a shape it does not model fails all of them at once,
loudly, instead of reading as a job with no runner and passing each of
them in silence. Split from ``workflow_inventory`` when the reader grew
past what that module could hold under the 400-line limit.
"""

from __future__ import annotations

import typing as typ

from runner_expressions import expression_labels


class UnreadableRunnerError(ValueError):
    """Raised when a job's ``runs-on`` is a shape the reader does not model.

    Returning no labels for such a job would read it as declaring no
    runner, which is what a reusable-workflow caller looks like, and every
    placement contract would then pass over it. A paid label inside the
    shape would bill with nothing checking it, so the reader refuses
    instead.
    """


#: The keys GitHub accepts in the mapping form of ``runs-on``.
_RUNNER_GROUP_KEYS = frozenset({"group", "labels"})

#: The prefix a runner group is reported under. A group is not a label, so
#: it matches neither the managed nor the hosted set and no actionlint
#: registration: every placement contract fails on it until one models it.
RUNNER_GROUP_PREFIX = "group:"


def runner_labels(job: dict[str, typ.Any]) -> frozenset[str]:
    """Return every runner label one job can select.

    A ``runs-on`` takes three forms: a label or an expression choosing
    between labels, a list of those, or a mapping with a ``group``, a
    ``labels`` entry, or both. The expression form is how a paid lane falls
    back for a pull request from a fork, which cannot obtain an Ubicloud
    runner. A reader that took the expression's text as a label would stop
    seeing either of the two it names. A reader that did not model the list
    or the mapping would read the job as declaring no runner. Either way
    every contract built on it would quietly assert nothing about that job.
    So anything outside the three forms is refused rather than read as
    empty, and so is an expression naming no label literal, such as
    ``${{ matrix.os }}``, which this reader cannot resolve.

    Parameters
    ----------
    job : dict[str, typing.Any]
        One parsed job.

    Returns
    -------
    frozenset[str]
        The labels the job can select, with a runner group reported as
        ``group:<name>``. Empty only when the job declares no ``runs-on``,
        as a reusable-workflow caller does.

    Raises
    ------
    UnreadableRunnerError
        If ``runs-on`` is present in a shape the reader does not model.

    Examples
    --------
    >>> runner_labels({"runs-on": "ubuntu-latest"}) == {"ubuntu-latest"}
    True
    >>> sorted(runner_labels({"runs-on": "${{ x && 'a' || 'b' }}"}))
    ['a', 'b']
    >>> sorted(runner_labels({"runs-on": {"group": "g", "labels": ["a"]}}))
    ['a', 'group:g']
    >>> runner_labels({"uses": "owner/repo/.github/workflows/w.yml@main"})
    frozenset()
    """
    match job.get("runs-on"):
        case None if "runs-on" not in job:
            return frozenset()
        case str() as runner:
            return _labels_in(runner)
        case list() as runners:
            return _labels_of_each(runners)
        case dict() as mapping if mapping and set(mapping) <= _RUNNER_GROUP_KEYS:
            return _labels_of_group(mapping)
        case other:
            raise _unreadable(other)


def _labels_in(runner: str) -> frozenset[str]:
    """Return the labels one ``runs-on`` string names, refusing an opaque one.

    An expression is read by :func:`runner_expressions.expression_labels`,
    which answers None when any result it can take is not a quoted label.
    """
    if not runner.strip():
        raise _unreadable(runner)
    if "${{" not in runner:
        return frozenset({runner})
    labels = expression_labels(runner)
    if labels is None:
        raise _unreadable(runner)
    return labels


def _labels_of_each(runners: object) -> frozenset[str]:
    """Return the labels a list form names, refusing an entry that is not text."""
    if not isinstance(runners, list) or not runners:
        raise _unreadable(runners)
    if not all(isinstance(entry, str) for entry in runners):
        raise _unreadable(runners)
    return frozenset().union(*(_labels_in(str(entry)) for entry in runners))


def _labels_of_group(mapping: dict[object, object]) -> frozenset[str]:
    """Return the labels and the group a mapping form names."""
    named: frozenset[str] = frozenset()
    if "labels" in mapping:
        labels = mapping["labels"]
        named = _labels_of_each([labels] if isinstance(labels, str) else labels)
    if "group" not in mapping:
        return named
    group = mapping["group"]
    if not isinstance(group, str) or not group:
        raise _unreadable(mapping)
    return named | {f"{RUNNER_GROUP_PREFIX}{group}"}


def _unreadable(runner: object) -> UnreadableRunnerError:
    """Return the refusal for one ``runs-on`` value."""
    return UnreadableRunnerError(
        f"runs-on {runner!r} is not a shape this reader models"
    )
