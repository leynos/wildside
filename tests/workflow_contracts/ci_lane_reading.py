"""Reads `ci.yml`'s jobs, steps and cache paths for the lane contracts.

Split out of :mod:`duplicate_test_lane_test` when that module crossed the
400-line limit the Python lint gate enforces, on the seam
:mod:`coverage_lanes` already uses: the queries here take a parsed
document and return what it declares, and the assertions stay with the
tests that make them.

One function opens a file, :func:`load_workflow`, and its caller names
the path. Everything else takes a parsed document and is pure, so a
contract that cannot read the workflow fails at the boundary that read
it rather than several frames inside a query.

The path is an argument rather than a module constant, and the queries
take a document rather than fetching one. A query that reached for
:data:`WORKFLOW_PATH` would make every call site's filesystem access
invisible and every signature quietly fallible, which is exactly what
:mod:`repository_reading` and :mod:`coverage_lanes` say in their own
docstrings that they exist to prevent. :data:`WORKFLOW_PATH` is offered
as data for a caller to pass, never read here.
"""

from __future__ import annotations

import typing as typ
from pathlib import Path

import repository_reading as reading
from timeout_budgets import COVERAGE_ACTION

WORKFLOW_LABEL = "ci.yml"

#: The job every "build job" contract here reads.
BUILD_JOB = "build"
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


def load_workflow(path: Path) -> dict[str, object]:
    """Return one parsed workflow document.

    The only function here that touches the filesystem, and the caller
    names the file it reads. Ordinarily that is
    :data:`WORKFLOW_PATH`, passed by a fixture so the read happens once
    per test and is visible in the test's own signature.

    Parameters
    ----------
    path : Path
        The workflow file to read.

    Returns
    -------
    dict[str, object]
        The whole document.

    Raises
    ------
    repository_reading.RepositoryReadError
        If the file cannot be read or is not valid YAML. Propagated
        rather than translated: it already carries the path, which is
        what an author needs, and wrapping it would hide the distinction
        between a file that could not be read and one whose shape is
        wrong.
    WorkflowShapeError
        If the file declares no mapping at its top level.
    """
    parsed = reading.parse_workflow(reading.read_text(path), path)
    if parsed is None:
        message = f"{path}: must declare a mapping at its top level"
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


def build_steps(document: dict[str, object]) -> list[dict[str, object]]:
    """Return the build job's steps.

    The only named shortcut for a single job, because three contracts
    want the same list. A matching `build_job` wrapper was removed: it
    said no more than `job_named(document, BUILD_JOB)` and read as a
    second, structurally identical helper.

    Parameters
    ----------
    document : dict[str, object]
        The parsed workflow.

    Returns
    -------
    list[dict[str, object]]
        The steps, in the order the job runs them.

    Raises
    ------
    WorkflowShapeError
        If the workflow declares no build job, or the job declares no
        list of steps, or one entry of it is not a mapping.
    """
    return steps_of(job_named(document, BUILD_JOB), BUILD_JOB)
