"""Reads `ci.yml`'s jobs, steps and cache paths for the lane contracts.

Split out of :mod:`duplicate_test_lane_test` when that module crossed the
400-line limit the Python lint gate enforces, on the seam
:mod:`coverage_lanes` already uses: the queries here take a parsed
document and return what it declares, and the assertions stay with the
tests that make them.

One function opens a file, :func:`ci_workflow`, and it names the path it
reads. Everything else is pure, so a contract that cannot read the
workflow fails at the boundary that read it rather than several frames
inside a query.
"""

from __future__ import annotations

import typing as typ
from pathlib import Path

import repository_reading as reading
from timeout_budgets import COVERAGE_ACTION

WORKFLOW_LABEL = "ci.yml"
WORKFLOW_PATH = (
    Path(__file__).resolve().parents[2] / ".github" / "workflows" / WORKFLOW_LABEL
)


class WorkflowShapeError(TypeError):
    """Raised when `ci.yml` does not have the shape a query needs.

    A shape fault is raised rather than asserted because this module is
    not a test file: the lint gate bans `assert` outside one, and an
    assertion would vanish under `python -O` while the query kept
    returning a value the caller cannot use. The message names the
    workflow and the part of it at fault, so a contract failing here
    says which file to open.
    """


def ci_workflow() -> dict[str, object]:
    """Return the parsed CI workflow.

    Returns
    -------
    dict[str, object]
        The whole document, read at the one named filesystem boundary
        the contract suite owns.

    Raises
    ------
    WorkflowShapeError
        If the file declares no mapping at its top level.
    """
    parsed = reading.parse_workflow(reading.read_text(WORKFLOW_PATH), WORKFLOW_PATH)
    if parsed is None:
        message = f"{WORKFLOW_LABEL} must declare a mapping at its top level"
        raise WorkflowShapeError(message)
    return dict(parsed)


def triggers_of(document: dict[str, object]) -> dict[str, object]:
    """Return the workflow's ``on`` mapping.

    YAML 1.1 parses a bare ``on`` key as the boolean ``True``, so the
    document is keyed on either spelling depending on how it was
    written.

    Parameters
    ----------
    document : dict[str, object]
        The parsed workflow.

    Returns
    -------
    dict[str, object]
        The triggers mapping.

    Raises
    ------
    WorkflowShapeError
        If the workflow declares no triggers mapping.
    """
    raw = document.get(True, document.get("on"))
    if not isinstance(raw, dict):
        message = f"{WORKFLOW_LABEL} must declare its triggers as a mapping"
        raise WorkflowShapeError(message)
    return typ.cast("dict[str, object]", raw)


def job_named(document: dict[str, object], job_name: str) -> dict[str, object]:
    """Return one job's mapping.

    Parameters
    ----------
    document : dict[str, object]
        The parsed workflow.
    job_name : str
        The job's identifier.

    Returns
    -------
    dict[str, object]
        The job.

    Raises
    ------
    WorkflowShapeError
        If the workflow declares no such job, or declares it as
        something other than a mapping.
    """
    jobs = document.get("jobs")
    if not isinstance(jobs, dict):
        message = f"{WORKFLOW_LABEL} must declare jobs as a mapping"
        raise WorkflowShapeError(message)
    job = jobs.get(job_name)
    if not isinstance(job, dict):
        message = f"{WORKFLOW_LABEL} must declare the {job_name} job as a mapping"
        raise WorkflowShapeError(message)
    return typ.cast("dict[str, object]", job)


def steps_of(job: dict[str, object], job_name: str) -> list[dict[str, object]]:
    """Return one job's steps, each checked to be a mapping.

    Parameters
    ----------
    job : dict[str, object]
        The parsed job.
    job_name : str
        The job's identifier, for the message.

    Returns
    -------
    list[dict[str, object]]
        The steps, in the order the job runs them.

    Raises
    ------
    WorkflowShapeError
        If the job declares no list of steps, or one entry of it is not
        a mapping.
    """
    steps = job.get("steps")
    if not isinstance(steps, list):
        message = f"the {job_name} job must declare its steps as a list"
        raise WorkflowShapeError(message)
    for index, step in enumerate(steps):
        if not isinstance(step, dict):
            message = f"{job_name} step {index} must be a mapping"
            raise WorkflowShapeError(message)
    return typ.cast("list[dict[str, object]]", steps)


def coverage_steps_of(job: dict[str, object]) -> list[dict[str, object]]:
    """Return the steps in one job that invoke the shared coverage action.

    Parameters
    ----------
    job : dict[str, object]
        The parsed job.

    Returns
    -------
    list[dict[str, object]]
        The matching steps.
    """
    return [
        step
        for step in steps_of(job, "coverage")
        if COVERAGE_ACTION in str(step.get("uses", ""))
    ]


def script_of(step: dict[str, object]) -> str | None:
    """Return a step's whole ``run`` script when it has one.

    Parameters
    ----------
    step : dict[str, object]
        The parsed step.

    Returns
    -------
    str or None
        The script, or None when the step runs an action instead.
    """
    run = step.get("run")
    return run if isinstance(run, str) else None


def cache_paths_of(step: dict[str, object]) -> list[str]:
    r"""Return a cache step's declared paths, ignoring its comment lines.

    `actions/cache` takes its paths as a block scalar and treats a
    `#`-prefixed line as a comment. Those lines are ordinary text to the
    YAML parser, so they are dropped here rather than searched: a comment
    naming a tool is not the job acquiring it.

    Parameters
    ----------
    step : dict[str, object]
        The parsed step.

    Returns
    -------
    list[str]
        The paths, or an empty list when the step is not a cache step.

    Examples
    --------
    >>> cache_paths_of({"run": "make deps"})
    []
    >>> cache_paths_of(
    ...     {
    ...         "uses": "actions/cache/restore@abc",
    ...         "with": {"path": "~/.cargo/bin\n# a note\n~/.theseus/postgresql\n"},
    ...     }
    ... )
    ['~/.cargo/bin', '~/.theseus/postgresql']
    """
    if "actions/cache" not in str(step.get("uses", "")):
        return []
    options = step.get("with")
    if not isinstance(options, dict):
        return []
    declared = str(options.get("path", ""))
    return [
        stripped
        for line in declared.splitlines()
        if (stripped := line.strip()) and not stripped.startswith("#")
    ]
