"""Contracts holding each paid job to the runner shape it was sized for.

`tool_installation_test.py` and the actionlint registry agree on which managed
labels exist, and that agreement is satisfied just as well by moving every job
and the registry back to `ubicloud-standard-8` together, which doubles the
rate with nothing to notice. So the shape is named here, job by job, and a
change to it has to edit this table and re-read the measurements in the
developers' guide ("Runner shapes") that justify it.

The table fails in both directions. A job on a managed runner with no entry
has had its cost decided by whoever typed the label, and an entry with no job
has been left behind by a rename.

Every sized job also runs the resource sampler, because the two figures a
shape is judged on, peak memory in use and minimum free disk, appear nowhere
else in a job's output. It starts after the checkout, which is what puts the
script on disk, and reports with `always()`, because a job its shape kills is
exactly the one whose peak explains the failure.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import typing as typ

import pytest
import workflow_inventory as inv

#: The managed shape each paid job was sized to. Keys are
#: ``(workflow filename, job id)``.
SIZED_JOBS: typ.Final[dict[tuple[str, str], str]] = {
    # Lint, Whitaker, the test suite and the compile-fail cases, compiled
    # through a warm sccache, so most compile time is cache fetch and link.
    ("ci.yml", "build"): "ubicloud-standard-4",
    # The serialized embedded-PostgreSQL suite dominates, and it is not
    # CPU-bound.
    ("ci.yml", "coverage"): "ubicloud-standard-4",
    # The same workload as `coverage`, on the trunk push.
    ("coverage-main.yml", "coverage-upload"): "ubicloud-standard-4",
}

SIZED_IDS = [f"{filename}:{job_id}" for filename, job_id in sorted(SIZED_JOBS)]

SAMPLER = "./scripts/ci-resource-sampler.sh"
CHECKOUT_ACTION = "actions/checkout"


def _managed_labels(job: dict[str, typ.Any]) -> frozenset[str]:
    """Return the managed labels a job can select, ignoring hosted fallbacks."""
    return inv.runner_labels(job) - inv.GITHUB_HOSTED_LABELS


def _managed_jobs() -> dict[tuple[str, str], dict[str, typ.Any]]:
    """Return every job in the estate that can select a managed runner."""
    return {
        (filename, job_id): job
        for filename, job_id, job in inv.iter_jobs()
        if _managed_labels(job)
    }


def _step_index(steps: list[dict[str, typ.Any]], run: str) -> int | None:
    """Return the index of the step whose whole command is `run`, if any."""
    matches = [index for index, step in enumerate(steps) if step.get("run") == run]
    return matches[0] if len(matches) == 1 else None


def test_every_managed_job_has_a_sizing_entry() -> None:
    """The table and the estate's paid jobs are the same set.

    A paid job missing from the table was sized by nobody; an entry with no
    job is stale.
    """
    managed = set(_managed_jobs())
    assert managed == set(SIZED_JOBS), (
        f"managed jobs without a sizing entry: {sorted(managed - set(SIZED_JOBS))}; "
        f"sizing entries without a managed job: {sorted(set(SIZED_JOBS) - managed)}"
    )


@pytest.mark.parametrize(("filename", "job_id"), sorted(SIZED_JOBS), ids=SIZED_IDS)
def test_each_managed_job_holds_the_shape_it_was_given(
    filename: str, job_id: str
) -> None:
    """A job selects exactly the managed shape its entry names."""
    job = inv.load_workflow(filename)["jobs"][job_id]
    expected = SIZED_JOBS[filename, job_id]
    assert _managed_labels(job) == {expected}, (
        f"{filename}:{job_id} selects {sorted(_managed_labels(job))}, but was "
        f"sized to {expected}; resize it here and in the developers' guide "
        "together, with the measurement that justifies it"
    )


@pytest.mark.parametrize(("filename", "job_id"), sorted(SIZED_JOBS), ids=SIZED_IDS)
def test_every_sized_job_samples_its_resources_after_the_checkout(
    filename: str, job_id: str
) -> None:
    """The sampler starts once, after the checkout that puts it on disk."""
    steps = inv.job_steps(inv.load_workflow(filename)["jobs"][job_id])
    start = _step_index(steps, f"{SAMPLER} start")
    checkouts = [
        index
        for index, step in enumerate(steps)
        if inv.step_action(step) == CHECKOUT_ACTION
    ]
    assert start is not None, (
        f"{filename}:{job_id} must run `{SAMPLER} start` as one step's whole command"
    )
    assert checkouts, f"{filename}:{job_id} has no checkout step"
    assert checkouts[0] < start, (
        f"{filename}:{job_id} starts the sampler before the checkout that "
        "puts the script on disk"
    )


@pytest.mark.parametrize(("filename", "job_id"), sorted(SIZED_JOBS), ids=SIZED_IDS)
def test_the_sampler_reports_even_when_the_job_fails(
    filename: str, job_id: str
) -> None:
    """The report is the last step and runs with `always()`.

    A success-only report loses the peak in the one run that needed it: the
    job its shape killed.
    """
    steps = inv.job_steps(inv.load_workflow(filename)["jobs"][job_id])
    report = _step_index(steps, f"{SAMPLER} report")
    assert report is not None, (
        f"{filename}:{job_id} must run `{SAMPLER} report` as one step's whole command"
    )
    assert steps[report].get("if") == "always()", (
        f"{filename}:{job_id} must report the sampler with `if: always()`"
    )
    assert report == len(steps) - 1, (
        f"{filename}:{job_id} must report the sampler last, so the peak covers "
        "every step"
    )
