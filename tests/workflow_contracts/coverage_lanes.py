"""Reads every coverage-invoking job out of the workflow files.

Separated from :mod:`timeout_budgets` so the workflow reading and the
nextest arithmetic stay legible apart, and so neither module outgrows
the 400-line limit the Python lint gate enforces.

The parsed documents are untyped as far as the YAML loader is concerned,
so every field this module reads is narrowed here, once, through the
guards below. Nothing downstream sees an unvalidated value.
"""

from __future__ import annotations

import typing as typ

import yaml
from timeout_budgets import COVERAGE_ACTION, WATCHDOG_VARIABLE, WORKFLOWS_DIRECTORY

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: One parsed YAML mapping, before any field of it has been read. The
#: values are ``object`` rather than ``typ.Any`` so a field cannot be
#: used without being narrowed first.
type Node = cabc.Mapping[str, object]


class WatchdogValueError(ValueError):
    """Raised when a workflow's watchdog value cannot be read as seconds.

    Distinguished from an unset watchdog rather than folded into it. A
    lane that sets nothing inherits the action's default, which is one
    fault; a lane that sets ``abc`` has an author who meant something
    and got neither, which is another. Reporting the second as the first
    would name the wrong remedy.
    """


def _mapping(value: object) -> Node | None:
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
    if not isinstance(value, dict):
        return None
    return {str(key): item for key, item in value.items()}


def _mappings_in(container: object) -> list[Node]:
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
    if not isinstance(container, list):
        return []
    return [mapping for item in container if (mapping := _mapping(item)) is not None]


def _budget_from(raw: object) -> float | None:
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


def watchdog_of(document: Node, job: Node, step: Node) -> float | None:
    """Return the watchdog budget in force for one step.

    All three levels are read, innermost first, as GitHub resolves them.
    Both workflows here set the value at job level, so a contract
    reading only the step would find nothing and report every lane as
    inheriting the action's default, which is exactly backwards. A
    workflow-level value would be missed the same way.

    Parameters
    ----------
    document : Node
        The whole workflow document.
    job : Node
        The enclosing job.
    step : Node
        The coverage step.

    Returns
    -------
    float or None
        The budget in seconds, or None when no level sets one.

    Raises
    ------
    WatchdogValueError
        If a level sets a value that is not a positive number of
        seconds.
    """
    for owner in (step, job, document):
        environment = _mapping(owner.get("env"))
        raw = None if environment is None else environment.get(WATCHDOG_VARIABLE)
        budget = _budget_from(raw)
        if budget is not None:
            return budget
    return None


class CoverageJob(typ.NamedTuple):
    """One job that invokes the coverage action, with its budgets.

    Attributes
    ----------
    workflow : str
        The workflow file's name.
    job : str
        The job's identifier.
    steps : int
        How many coverage steps the job runs. Each gets its own
        watchdog, so the job must contain all of their budgets.
    watchdogs : tuple[float | None, ...]
        The watchdog budget in force for each of those steps, in order,
        with None where no level sets one.
    job_timeout : float or None
        The job's ``timeout-minutes`` in seconds, or None when it
        declares none and so inherits GitHub's six-hour default.
    conditions : tuple[tuple[object, object], ...]
        The ``if`` on each coverage step and on its job, in step order.
        A skipped step runs no ``cargo``, so its watchdog never arms and
        the tiers say nothing about it; the condition is part of what
        identifies a lane rather than incidental to it.
    """

    workflow: str
    job: str
    steps: int
    watchdogs: tuple[float | None, ...]
    job_timeout: float | None
    conditions: tuple[tuple[object, object], ...] = ()

    def __str__(self) -> str:
        """Return a location suitable for a failure message.

        Returns
        -------
        str
            ``workflow:job`` for this job.
        """
        return f"{self.workflow}:{self.job}"


def workflow_documents() -> dict[str, Node]:
    """Return every workflow document in the repository, keyed by name.

    This is the one place the contract touches the filesystem, so an
    unreadable or unparsable workflow fails here rather than inside a
    budget derivation. Both extensions are read; a coverage lane in the
    other one would otherwise escape every assertion without failing
    anything.

    Returns
    -------
    dict[str, Node]
        File name to parsed document.
    """
    documents: dict[str, Node] = {}
    for pattern in ("*.yml", "*.yaml"):
        for path in sorted(WORKFLOWS_DIRECTORY.glob(pattern)):
            parsed = _mapping(yaml.safe_load(path.read_text(encoding="utf-8")))
            if parsed is not None:
                documents[path.name] = parsed
    return documents


def _coverage_steps(job: Node) -> list[Node]:
    """Return the steps in one job that invoke the coverage action.

    Parameters
    ----------
    job : Node
        The parsed job.

    Returns
    -------
    list[Node]
        The matching steps, in the order the job runs them.
    """
    return [
        step
        for step in _mappings_in(job.get("steps"))
        if COVERAGE_ACTION in str(step.get("uses", ""))
    ]


def _job_ceiling(job: Node) -> float | None:
    """Return the job's ``timeout-minutes`` in seconds, or None.

    Parameters
    ----------
    job : Node
        The parsed job.

    Returns
    -------
    float or None
        The ceiling in seconds, or None when the job declares none.
    """
    raw = job.get("timeout-minutes")
    if raw is None:
        return None
    return float(str(raw)) * 60.0


def _coverage_job(
    workflow: str, document: Node, job_name: str, job: Node
) -> CoverageJob | None:
    """Return one job's budgets, or None when it runs no coverage step.

    Parameters
    ----------
    workflow : str
        The workflow file's name.
    document : Node
        The enclosing document, read for a workflow-level watchdog.
    job_name : str
        The job's identifier.
    job : Node
        The parsed job.

    Returns
    -------
    CoverageJob or None
        The job's budgets, or None when it invokes no coverage step.

    Raises
    ------
    WatchdogValueError
        If a step's watchdog value cannot be read as a positive number
        of seconds. The lane's coordinate is added to the message, so
        the failure names the workflow and job at fault rather than
        reporting a bare conversion error.
    """
    steps = _coverage_steps(job)
    if not steps:
        return None
    try:
        watchdogs = tuple(watchdog_of(document, job, step) for step in steps)
    except WatchdogValueError as error:
        message = f"{workflow}:{job_name}: {error}"
        raise WatchdogValueError(message) from error
    return CoverageJob(
        workflow=workflow,
        job=job_name,
        steps=len(steps),
        watchdogs=watchdogs,
        job_timeout=_job_ceiling(job),
        conditions=tuple((step.get("if"), job.get("if")) for step in steps),
    )


def _declared_jobs(
    documents: cabc.Mapping[str, Node],
) -> list[tuple[str, Node, str, Node]]:
    """Return every job in every workflow, carrying its file and document.

    Parameters
    ----------
    documents : cabc.Mapping[str, Node]
        Parsed workflow documents, keyed by file name.

    Returns
    -------
    list of tuple
        Workflow name, document, job identifier, and job.
    """
    declared: list[tuple[str, Node, str, Node]] = []
    for name, document in documents.items():
        jobs = _mapping(document.get("jobs"))
        if jobs is None:
            continue
        declared.extend(
            (name, document, job_name, parsed)
            for job_name, job in jobs.items()
            if (parsed := _mapping(job)) is not None
        )
    return declared


def coverage_jobs_of(
    documents: cabc.Mapping[str, Node] | None = None,
) -> tuple[CoverageJob, ...]:
    """Return every job invoking the coverage action, with its budgets.

    Jobs are the unit rather than steps, because the ceiling is a job's
    and it has to contain every watchdog inside it. Counting steps is
    what makes two invocations in one job visible to the arithmetic.

    The documents are a parameter so the reading can be driven with
    synthetic workflows, which keeps the filesystem access at one named
    boundary instead of inside the derivation.

    Parameters
    ----------
    documents : cabc.Mapping[str, Node] or None
        Parsed workflow documents keyed by file name. When None, the
        repository's own ``.github/workflows`` is read.

    Returns
    -------
    tuple[CoverageJob, ...]
        One entry per coverage-invoking job.
    """
    if documents is None:
        documents = workflow_documents()
    return tuple(
        found
        for workflow, document, job_name, job in _declared_jobs(documents)
        if (found := _coverage_job(workflow, document, job_name, job)) is not None
    )
