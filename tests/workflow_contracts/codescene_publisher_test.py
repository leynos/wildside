"""Contracts on the one workflow allowed to publish coverage.

Split from `codescene_coverage_baseline_test.py` when that module
crossed the 400-line limit the Python lint gate enforces. The seam is
the question: that module says what no pull-request workflow may do, and
this one says what the publisher must do.

The two halves are not symmetrical, and the asymmetry is the point. The
absences can all be satisfied by deleting things; requiring a publisher
that actually uploads, from the trunk ref, without racing itself, is
what stops the baseline being satisfied into nonexistence. A ratchet
comparing every pull request against a baseline nothing advances passes
quietly and protects nothing.
"""

from __future__ import annotations

import workflow_inventory as inventory
from codescene_baseline import (
    CODESCENE_ACTION_MARKER,
    PUBLISHER,
    PUBLISHER_BRANCHES,
    PUBLISHER_CONCURRENCY_GROUP,
    PUBLISHER_EVENT,
    PUBLISHER_UPLOAD_CONDITION,
    UPLOAD_MODE,
)


def _uploads(step: dict[str, object]) -> bool:
    """Return whether a CodeScene step publishes rather than checks.

    The effective mode is read, not the declared one. `mode` is optional
    on `upload-codescene-coverage` and defaults to `upload`, and the
    publisher omits it, so a contract requiring the literal input would
    fail on a correct workflow and one reading only a present input
    would pass on `mode: check`.

    Parameters
    ----------
    step : dict[str, object]
        A step already known to use a CodeScene action.

    Returns
    -------
    bool
        True when the step's effective mode is `upload`.
    """
    options = step.get("with")
    declared = (
        options.get("mode", UPLOAD_MODE) if isinstance(options, dict) else (UPLOAD_MODE)
    )
    return declared == UPLOAD_MODE


def test_the_push_publisher_is_the_one_workflow_that_uploads() -> None:
    """Coverage reaches CodeScene from the trunk, and only from there.

    Scenario: the publisher workflow. Invariant: it runs on a push, it is
    not a pull-request workflow, and it calls a CodeScene action.

    All three clauses are load-bearing together. Without the last, the
    baseline could be satisfied by deleting the upload entirely, which
    would leave CodeScene with no coverage data at all and the ratchet
    comparing pull requests against a baseline nothing advances. Without
    the second, the publisher could acquire a `pull_request` trigger and
    put the secret back on every pull request by another door.
    """
    triggers = inventory.triggers_of(PUBLISHER)
    assert PUBLISHER_EVENT in triggers, (
        f"{PUBLISHER} must run on {PUBLISHER_EVENT}; it is the only lane that "
        "advances the coverage baseline the pull-request ratchet reads"
    )
    push = triggers[PUBLISHER_EVENT]
    assert isinstance(push, dict), (
        f"{PUBLISHER}'s {PUBLISHER_EVENT} trigger must declare a mapping so it "
        f"can name its branches, found {push!r}"
    )
    assert push.get("branches") == PUBLISHER_BRANCHES, (
        f"{PUBLISHER}'s {PUBLISHER_EVENT} trigger must select exactly "
        f"{PUBLISHER_BRANCHES}, found {push!r}; an unrestricted trigger lets a "
        "feature branch advance the baseline, and every pull request is then "
        "ratcheted against coverage that is not the trunk's"
    )
    assert PUBLISHER not in inventory.pull_request_workflows(), (
        f"{PUBLISHER} holds the CodeScene secret, so it must never run on a "
        "pull request"
    )
    uploads = [
        step
        for _, job in inventory.workflow_jobs(PUBLISHER)
        for step in inventory.job_steps(job)
        if CODESCENE_ACTION_MARKER in inventory.step_action(step).lower()
    ]
    assert uploads, (
        f"{PUBLISHER} must call a CodeScene action; with no publisher the "
        "ratchet compares every pull request against a baseline that nothing "
        "advances, and CodeScene sees no coverage for this repository at all"
    )
    publishing = [step.get("name") for step in uploads if _uploads(step)]
    assert publishing, (
        f"{PUBLISHER} must call the CodeScene action in {UPLOAD_MODE!r} mode; "
        f"{[step.get('name') for step in uploads]} calls it in another mode, "
        "which sends no coverage and leaves the baseline unadvanced while "
        "every other assertion here still passes"
    )


def test_the_publisher_uploads_only_from_the_main_ref() -> None:
    """Coverage is published from the trunk ref and from nowhere else.

    Scenario: the publisher runs on a push to `main` and on
    `workflow_dispatch`, and the dispatch trigger is mandatory here
    because merges made by the automerge workflow's token do not fire
    push events. A dispatch can be started from any branch. Invariant:
    the upload step carries the reviewed condition, both clauses.

    The ref clause is not redundant with the push branch filter, and
    that is the whole reason this case exists. The branch filter
    constrains one of the two triggers; a dispatch from a feature branch
    passes it untouched, and without the ref clause that branch's
    coverage would be uploaded as the trunk's and become the baseline
    every pull request is ratcheted against. Nothing in the rollup would
    say so: the upload succeeds.
    """
    uploads = [
        step
        for _, job in inventory.workflow_jobs(PUBLISHER)
        for step in inventory.job_steps(job)
        if CODESCENE_ACTION_MARKER in inventory.step_action(step).lower()
    ]
    assert uploads, f"{PUBLISHER} must call a CodeScene action"
    conditions = [step.get("if") for step in uploads]
    assert conditions == [PUBLISHER_UPLOAD_CONDITION], (
        f"every upload step in {PUBLISHER} must carry "
        f"{PUBLISHER_UPLOAD_CONDITION!r}; found {conditions}. The token clause "
        "alone lets a dispatch from any branch publish that branch's coverage "
        "as the trunk's"
    )


def test_the_publisher_serializes_its_baseline_writes() -> None:
    """Two publishers cannot race to write the baseline.

    Scenario: two pushes land on `main` close together, or a push and a
    dispatch overlap. Invariant: the workflow declares a concurrency
    group, and does not cancel a run in progress.

    Both halves matter and they pull opposite ways. Without a group the
    two runs race, and the baseline every pull request reads afterwards
    is whichever finished second, decided by runner scheduling. With
    `cancel-in-progress: true` the newer run kills the older one
    mid-write, which is the same lost update arrived at deliberately. A
    pull-request lane may cancel itself; a trunk publisher may not.
    """
    document = inventory.load_workflow(PUBLISHER)
    concurrency = document.get("concurrency")
    assert isinstance(concurrency, dict), (
        f"{PUBLISHER} must declare a concurrency mapping; without one two "
        "pushes to main race to write the baseline and the winner is decided "
        f"by runner scheduling, found {concurrency!r}"
    )
    assert concurrency.get("group") == PUBLISHER_CONCURRENCY_GROUP, (
        f"{PUBLISHER}'s concurrency group must be "
        f"{PUBLISHER_CONCURRENCY_GROUP!r}, found {concurrency.get('group')!r}"
    )
    assert concurrency.get("cancel-in-progress") is False, (
        f"{PUBLISHER} must not cancel a run in progress, found "
        f"{concurrency.get('cancel-in-progress')!r}; cancelling a publisher "
        "abandons a baseline write half done, which is the lost update the "
        "group exists to prevent"
    )
