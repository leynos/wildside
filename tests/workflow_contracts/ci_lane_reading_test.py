"""Unit tests for the workflow reader's shape faults.

The lane contracts ask :mod:`ci_lane_reading` about the one workflow
this repository has, so they only ever exercise the paths where the
shape is right. Every `raise` in that module is therefore reached by
nothing else, and an untested `raise` is a message no one has read: the
point of raising rather than returning None is that a contract failing
inside a query says which file to open and which part of it is at fault.

The module raises rather than asserts because it is not a test file. The
lint gate bans `assert` outside one, and an assertion would vanish under
`python -O` while the query kept returning a value its caller cannot
use. That reasoning is only worth anything if the exceptions fire, so
each is driven here from a document built to be wrong in exactly one
way.

The messages are asserted as substrings naming the workflow and the part
at fault, not verbatim. Pinning the whole sentence would make a reworded
message a failure; asserting nothing would let the module raise a bare
`TypeError` with no path in it, which is the state this replaced.
"""

from __future__ import annotations

import typing as typ

import ci_lane_reading as lanes
import pytest
import repository_reading as reading

if typ.TYPE_CHECKING:
    from pathlib import Path


def test_a_document_with_no_mapping_at_the_top_level_is_a_shape_fault(
    tmp_path: Path,
) -> None:
    """A workflow that parses to something other than a mapping is refused.

    Scenario: the file is valid YAML but declares a list, which is what
    a half-finished edit or a stray indent produces. Invariant: the
    loader raises a shape fault naming the path. Returning the list
    would push the failure into whichever query indexed it first, and
    the message there would name a key rather than the file.
    """
    workflow = tmp_path / "ci.yml"
    workflow.write_text("- build\n- coverage\n", encoding="utf-8")

    with pytest.raises(lanes.WorkflowShapeError, match="top level"):
        lanes.load_workflow(workflow)


def test_an_unreadable_file_keeps_its_own_error(tmp_path: Path) -> None:
    """A file that cannot be read fails as a read error, not a shape one.

    Scenario: the path does not exist. Invariant: the reader's own
    `RepositoryReadError` propagates. The distinction is deliberate and
    documented in the loader: a file that could not be read and a file
    whose shape is wrong are different faults, and wrapping the first in
    the second would tell an author to go and look at a file that is not
    there.
    """
    with pytest.raises(reading.RepositoryReadError):
        lanes.load_workflow(tmp_path / "absent.yml")


@pytest.mark.parametrize(
    "document",
    [
        pytest.param({"jobs": {}}, id="no-triggers-key"),
        pytest.param({"on": "pull_request"}, id="triggers-as-a-string"),
        pytest.param({"on": ["pull_request"]}, id="triggers-as-a-list"),
        pytest.param({True: None}, id="a-valueless-on-key"),
    ],
)
def test_triggers_must_be_a_mapping(document: dict[str, object]) -> None:
    """Triggers declared as anything but a mapping are a shape fault.

    Scenario: `on` is absent, or is a string or list, both of which
    GitHub accepts and this reader does not. Invariant: the query
    raises. A list is the case that matters: `in` would answer on it,
    so a reader that returned it would let a contract asking whether a
    trigger is declared quietly keep working until the day it is not.
    """
    with pytest.raises(lanes.WorkflowShapeError, match="triggers"):
        lanes.triggers_of(document)


def test_triggers_read_either_spelling_of_the_on_key() -> None:
    """Both the boolean and the string spelling of `on` are found.

    Scenario: YAML 1.1 parses a bare `on` key as the boolean `True`, and
    a quoted `"on"` stays a string. Invariant: the query finds the
    mapping either way. This is the companion to the fault cases: the
    reason the fault path exists is that the key has two spellings and a
    reader that knew one would raise on a perfectly good workflow.
    """
    boolean_key: dict[str, object] = typ.cast(
        "dict[str, object]", {True: {"pull_request": None}}
    )
    assert lanes.triggers_of(boolean_key) == {"pull_request": None}
    assert lanes.triggers_of({"on": {"push": None}}) == {"push": None}


@pytest.mark.parametrize(
    ("document", "expected"),
    [
        pytest.param({}, "jobs", id="no-jobs-key"),
        pytest.param({"jobs": []}, "jobs", id="jobs-as-a-list"),
        pytest.param({"jobs": {"coverage": {}}}, "build", id="the-job-is-absent"),
        pytest.param({"jobs": {"build": None}}, "build", id="a-valueless-job"),
        pytest.param({"jobs": {"build": "yes"}}, "build", id="a-job-as-a-string"),
    ],
)
def test_a_missing_or_misshapen_job_is_a_shape_fault(
    document: dict[str, object], expected: str
) -> None:
    """Asking for a job the workflow does not declare raises.

    Scenario: the jobs mapping is missing or is a list, or the named job
    is absent or is not a mapping. Invariant: the query raises and the
    message names the part at fault. A query returning an empty mapping
    instead would make every contract about that job pass vacuously,
    which is the exact failure mode these contracts were written to
    avoid elsewhere.
    """
    with pytest.raises(lanes.WorkflowShapeError, match=expected):
        lanes.job_named(document, lanes.BUILD_JOB)


@pytest.mark.parametrize(
    ("job", "expected"),
    [
        pytest.param({}, "list", id="no-steps-key"),
        pytest.param({"steps": None}, "list", id="a-valueless-steps-key"),
        pytest.param({"steps": {"run": "make test"}}, "list", id="steps-as-a-mapping"),
    ],
)
def test_steps_must_be_a_list(job: dict[str, object], expected: str) -> None:
    """A job that declares no list of steps raises rather than reads as empty.

    Scenario: `steps` is absent, valueless, or a mapping. Invariant: the
    query raises. An empty list would satisfy every "no unguarded step"
    assertion in the lane contracts at once, and satisfying them by
    reading nothing is indistinguishable from satisfying them honestly.
    """
    with pytest.raises(lanes.WorkflowShapeError, match=expected):
        lanes.steps_of(job, lanes.BUILD_JOB)


@pytest.mark.parametrize(
    ("steps", "index"),
    [
        pytest.param([None], 0, id="a-valueless-first-step"),
        pytest.param([{"run": "make lint"}, "make test"], 1, id="a-step-as-a-string"),
        pytest.param([{"run": "a"}, {"run": "b"}, []], 2, id="a-step-as-a-list"),
    ],
)
def test_a_step_that_is_not_a_mapping_is_named_by_index(
    steps: list[object], index: int
) -> None:
    """A malformed step is refused, and the message says which one.

    Scenario: one entry of an otherwise good list is not a mapping.
    Invariant: the query raises and names that entry's index. The index
    is the whole value of the message here: the steps have no keys to
    quote until they parse, so an author has nothing else to search the
    file for.
    """
    with pytest.raises(lanes.WorkflowShapeError, match=f"step {index}"):
        lanes.steps_of({"steps": steps}, lanes.BUILD_JOB)


def test_the_build_step_query_propagates_the_fault_it_meets() -> None:
    """The build-job shortcut raises for either of the faults beneath it.

    Scenario: the shortcut composes `job_named` and `steps_of`, so it
    has two ways to fail and no error handling of its own. Invariant:
    the fault from whichever query met it reaches the caller unchanged.
    A shortcut that swallowed one and returned an empty list would make
    three contracts pass on a workflow with no build job at all.
    """
    with pytest.raises(lanes.WorkflowShapeError, match=lanes.BUILD_JOB):
        lanes.build_steps({"jobs": {"coverage": {}}})
    with pytest.raises(lanes.WorkflowShapeError, match="list"):
        lanes.build_steps({"jobs": {lanes.BUILD_JOB: {}}})


@pytest.mark.parametrize(
    "step",
    [
        pytest.param({"run": "make deps"}, id="a-run-step"),
        pytest.param({"uses": "actions/checkout@v4"}, id="an-unrelated-action"),
        pytest.param({"uses": "actions/cache@v4"}, id="a-cache-step-with-no-inputs"),
        pytest.param(
            {"uses": "actions/cache@v4", "with": "path"}, id="inputs-as-a-string"
        ),
        pytest.param(
            {"uses": "actions/cache@v4", "with": {}}, id="inputs-with-no-path"
        ),
    ],
)
def test_a_step_declaring_no_cache_paths_reads_as_none(
    step: dict[str, object],
) -> None:
    """Cache paths are absent rather than faulty when the step has none.

    Scenario: the step is not a cache step, or is one whose inputs are
    missing or misshapen. Invariant: the query returns an empty list and
    does not raise. This one is deliberately not a shape fault: the lane
    contract asks every build step for its cache paths, and most steps
    honestly have none.
    """
    assert lanes.cache_paths_of(step) == []
