"""Contracts on which actor and event reaches a backend suite lane.

`duplicate_test_lane_test.py` pins each lane's condition verbatim and
asserts the two against each other. That catches a narrowed string, but
it is still a comparison of text: it says the conditions are the agreed
ones, not what the agreed ones do. The hole this branch nearly shipped
was of exactly the second kind. Both conditions read reasonably on their
own, and the fault was that together they left one actor with no lane.

So these contracts evaluate the conditions instead. Every step that runs
the backend suite is discovered from the workflows, its job and step
conditions are evaluated for a given actor and event, and the lanes that
would actually run are counted. Two invariants follow, and they pull in
opposite directions:

- **Safety.** Every actor and event that starts CI runs the suite
  somewhere. This is the invariant whose absence was the defect: it
  fails outright if the Dependabot guard is dropped from the build lane
  while the coverage job still excludes that actor.
- **Economy.** A pull request runs it exactly once. This is the
  invariant the branch exists to establish, and it fails if the build
  lane's guard is widened back to every contributor.

Three lanes are in play, because the push case lives in a second
workflow. `ci.yml`'s coverage job runs on a pull request for everyone
but Dependabot; `ci.yml`'s build job runs it for Dependabot alone; and
`coverage-main.yml` runs it on a push to `main`, which is why `ci.yml`
skips coverage there. A contract reading only `ci.yml` would conclude
that a push runs no suite at all.

The Dependabot-on-push combination is deliberately held to safety only.
An automerged dependency push can attribute to `dependabot[bot]`, which
reaches both the build lane and `coverage-main.yml`; that is two runs
rather than none, so it is a cost question and not a hole, and pinning
it here would make an unrelated change to the automerge route fail a
safety contract.
"""

from __future__ import annotations

import itertools

import ci_lane_reading as lanes
import ci_step_predicates as does
import github_conditions as conditions
import pytest
from lane_expectations import COMPILE_FAIL_SCRIPT
from timeout_budgets import COVERAGE_ACTION

#: The workflow that runs the suite on a push to `main`. `ci.yml` skips
#: its coverage job on push precisely because this one covers it.
MAIN_COVERAGE_LABEL = "coverage-main.yml"

#: Actors worth distinguishing: the one both lanes' conditions name, and
#: an ordinary contributor standing for everyone else.
ACTORS = ("dependabot[bot]", "octocat")

#: The events `ci.yml` declares. `workflow_dispatch` is included because
#: it is a trigger the coverage job's `event_name != 'push'` clause lets
#: through, and a lane reachable on it is a lane that has to be counted.
EVENTS = ("pull_request", "push", "workflow_dispatch")


def _workflow(label: str) -> dict[str, object]:
    """Return one parsed workflow from the repository's workflow directory.

    Parameters
    ----------
    label : str
        The workflow file's name.

    Returns
    -------
    dict[str, object]
        The parsed document.
    """
    return lanes.load_workflow(lanes.WORKFLOW_PATH.with_name(label))


def _runs_the_suite(step: dict[str, object]) -> bool:
    """Return whether a step runs the backend suite by either route.

    The shared coverage action runs it through nextest without naming a
    command, so a script predicate alone cannot see it. The compile-fail
    step is excluded: it runs two named binaries that neither suite run
    ever executes, and counting it would report a suite lane on every
    event.

    Parameters
    ----------
    step : dict[str, object]
        The parsed step.

    Returns
    -------
    bool
        True when the step runs the suite.
    """
    if lanes.script_of(step) == COMPILE_FAIL_SCRIPT:
        return False
    if COVERAGE_ACTION in str(step.get("uses", "")):
        return True
    return does.runs_the_suite(step)


def _condition(owner: dict[str, object], label: str) -> str | None:
    """Return one job's or step's `if` expression, checked to be a string.

    A shape fault rather than a coercion. GitHub accepts `if: true`,
    which parses to a boolean, and passing that to the evaluator would
    raise a syntax error naming a grammar rather than naming the
    workflow.

    Parameters
    ----------
    owner : dict[str, object]
        The parsed job or step.
    label : str
        What to call it in the message.

    Returns
    -------
    str or None
        The condition, or None when it declares none.

    Raises
    ------
    ci_lane_reading.WorkflowShapeError
        If the condition is declared as something other than a string.
    """
    value = owner.get("if")
    if value is None or isinstance(value, str):
        return value
    message = f"{label}: an `if` must be a string, found {value!r}"
    raise lanes.WorkflowShapeError(message)


def _reachable_lanes(actor: str, event: str) -> list[str]:
    """Return the suite lanes that would run for one actor and event.

    Parameters
    ----------
    actor : str
        The value of `github.actor`.
    event : str
        The value of `github.event_name`.

    Returns
    -------
    list[str]
        One label per running lane, as `workflow/job/step`.
    """
    context = {"actor": actor, "event_name": event}
    running: list[str] = []
    for label in (lanes.WORKFLOW_LABEL, MAIN_COVERAGE_LABEL):
        document = _workflow(label)
        if event not in lanes.triggers_of(document):
            continue
        running += _running_steps(document, label, context)
    return running


def _running_steps(
    document: dict[str, object], label: str, context: dict[str, str]
) -> list[str]:
    """Return the suite steps one workflow would run in a context.

    Parameters
    ----------
    document : dict[str, object]
        The parsed workflow.
    label : str
        The workflow's name, for the returned labels.
    context : dict[str, str]
        The `github` fields the conditions reference.

    Returns
    -------
    list[str]
        One label per running suite step, as `workflow/job/step`.
    """
    jobs = document.get("jobs")
    if not isinstance(jobs, dict):
        message = f"{label} must declare jobs as a mapping"
        raise lanes.WorkflowShapeError(message)
    running: list[str] = []
    for raw_name in jobs:
        job_name = str(raw_name)
        job = lanes.job_named(document, job_name)
        if not conditions.evaluate(_condition(job, f"{label}/{job_name}"), context):
            continue
        running += [
            f"{label}/{job_name}/{step.get('name')}"
            for step in lanes.steps_of(job, job_name)
            if _runs_the_suite(step)
            and conditions.evaluate(
                _condition(step, f"{label}/{job_name}/{step.get('name')}"), context
            )
        ]
    return running


@pytest.mark.parametrize(("actor", "event"), list(itertools.product(ACTORS, EVENTS)))
def test_every_actor_and_event_reaches_a_backend_suite(actor: str, event: str) -> None:
    """No actor on any triggering event is left without a suite run.

    Scenario: each actor the lane conditions distinguish, on each event
    the workflows trigger on. Invariant: at least one lane runs the
    backend suite. This is the defect stated as a contract: deleting the
    build job's step, or dropping its Dependabot guard, leaves
    `dependabot[bot]` on a pull request with lints and two compile-fail
    commands and no suite, then merging itself on green.

    Unlike the complementarity contract beside it, this one evaluates
    the conditions rather than comparing them, so it fails for a pair of
    individually reasonable conditions that together exclude someone.
    """
    running = _reachable_lanes(actor, event)
    assert running, (
        f"{actor} on {event} reaches no lane that runs the backend suite; "
        "every triggering combination must reach one, or a pull request "
        "merges with the backend untested"
    )


@pytest.mark.parametrize("actor", ACTORS)
def test_a_pull_request_runs_the_backend_suite_exactly_once(actor: str) -> None:
    """A pull request runs the suite once, whoever opened it.

    Scenario: a pull request from Dependabot and from an ordinary
    contributor. Invariant: exactly one lane runs the backend suite.
    This is what the branch exists to establish, and it is the direction
    the safety contract above cannot see: widening the build lane's
    guard back to every contributor satisfies safety and fails here.
    """
    running = _reachable_lanes(actor, "pull_request")
    assert len(running) == 1, (
        f"{actor} on a pull request reaches {len(running)} suite lanes, "
        f"{running}; a pull request must run the backend suite exactly once"
    )


def test_a_push_to_main_runs_the_suite_in_the_other_workflow() -> None:
    """The push lane is `coverage-main.yml`, not `ci.yml`.

    Scenario: an ordinary push to `main`, where `ci.yml` skips its
    coverage job and its build job's suite step is Dependabot-only.
    Invariant: the lane that runs is the one in `coverage-main.yml`.
    Asserted by name rather than by count because the reason `ci.yml`
    skips coverage on push is that this workflow covers it; if that
    workflow stopped running the suite, `ci.yml`'s skip would become a
    hole and the count alone would still be satisfied by any lane.
    """
    running = _reachable_lanes("octocat", "push")
    assert [lane.split("/")[0] for lane in running] == [MAIN_COVERAGE_LABEL], (
        f"a push to main must run the backend suite in {MAIN_COVERAGE_LABEL}, "
        f"found {running}"
    )


def test_an_unevaluable_condition_stops_the_contract() -> None:
    """A condition outside the grammar raises rather than reading as false.

    Scenario: a lane guarded by an expression this evaluator does not
    know, such as one using `||` or a function call. Invariant: it
    raises. A partial evaluator returning False would report the lane as
    unreachable, and the safety contract above would then fail for a
    reason that has nothing to do with the workflow, or the economy
    contract would pass because a real lane went uncounted.
    """
    context = {"actor": "octocat", "event_name": "pull_request"}
    with pytest.raises(conditions.ConditionSyntaxError, match="outside the grammar"):
        conditions.evaluate("success() || github.actor == 'octocat'", context)
    with pytest.raises(conditions.ConditionSyntaxError, match="no value for"):
        conditions.evaluate("github.ref == 'refs/heads/main'", context)

    # An unsupported term *after* a false one. This is the case a
    # short-circuiting `all()` never reaches: it would answer False, and the
    # lane would be reported as simply not running rather than as guarded by
    # an expression nothing here can read.
    with pytest.raises(conditions.ConditionSyntaxError, match="outside the grammar"):
        conditions.evaluate("github.actor == 'nobody' && success()", context)
