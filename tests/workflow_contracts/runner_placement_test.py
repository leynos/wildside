"""Every paid lane a fork can reach must fall back to a hosted runner.

Ubicloud runners are not available to a pull request from a fork. A job
that names one unconditionally therefore has no runner at all on a fork's
pull request, and the lane fails for a reason that has nothing to do with
the change under review. The remedy is an event-keyed label: the fork
takes a GitHub-hosted runner and every other event keeps the managed one.

The rule is asserted over the estate rather than over the two jobs that
carry it today. A contract naming those two would say nothing about the
third Ubicloud lane somebody adds, which is the one that would break.

Jobs in workflows a fork cannot trigger are deliberately outside the
rule. `coverage-main.yml` runs on a push to `main`, which a fork cannot
perform, so its unconditional Ubicloud label is correct and a contract
that demanded a fallback there would be demanding a branch that can never
be taken.

The raw declaration is checked as well as the expression it parses to.
A folded scalar whose continuation is indented deeper than its first
line keeps the break rather than folding it, so the value GitHub
evaluates has a newline inside it. GitHub evaluates it anyway, which is
why a green run says nothing and the shape has to be refused here.
"""

from __future__ import annotations

import re
import typing as typ

import pytest
import workflow_inventory as inv

#: The expression field that is true only for a pull request from a fork.
FORK_FIELD: typ.Final[str] = "github.event.pull_request.head.repo.fork"

#: The runner a fork falls back to. GitHub-hosted minutes are free on a
#: public repository, so the fallback costs nothing beyond its wall clock.
FORK_RUNNER: typ.Final[str] = "ubuntu-latest"

#: How an Ubicloud label is recognized, wherever it appears.
UBICLOUD_PREFIX: typ.Final[str] = "ubicloud-"

#: The whole `runs-on` value, parsed rather than matched. Every part is
#: captured: a contract that searched for the fork field as a substring
#: would pass on an expression that named it and then ignored it, and one
#: that searched for `ubuntu-latest` would pass with the two arms swapped.
_FALLBACK = re.compile(
    r"^\$\{\{\s*(?P<condition>[A-Za-z0-9_.]+)\s*"
    r"&&\s*'(?P<fork>[^']*)'\s*"
    r"\|\|\s*'(?P<trusted>[^']*)'\s*\}\}$"
)


def _triggers(filename: str) -> object:
    """Return one workflow's `on` value, however the file spells the key.

    YAML 1.1 parses an unquoted `on:` to the boolean `True`, so a reader
    that looked only for the string would find nothing in exactly the
    files that did not quote it.
    """
    document = inv.load_workflow(filename)
    for key in ("on", True):
        if key in document:
            return document[key]
    return {}


def _reachable_by_a_fork(filename: str) -> bool:
    """Return whether a fork's pull request can trigger this workflow.

    `pull_request_target` is deliberately not counted. It runs against the
    base repository with its own runner allocation and is not a lane a
    fork's own pull request can place work on.
    """
    triggers = _triggers(filename)
    if isinstance(triggers, dict):
        return "pull_request" in triggers
    if isinstance(triggers, list):
        return "pull_request" in triggers
    return triggers == "pull_request"


def _runner_text(job: dict[str, typ.Any]) -> str:
    """Return a job's `runs-on` as one string, whatever shape it has."""
    runner = job.get("runs-on")
    if isinstance(runner, str):
        return runner
    if isinstance(runner, list):
        return " ".join(str(entry) for entry in runner)
    return ""


def _paid_lanes() -> list[tuple[str, str, str]]:
    """Return every Ubicloud lane a fork's pull request can reach."""
    return [
        (filename, job_id, _runner_text(job))
        for filename, job_id, job in inv.iter_jobs()
        if UBICLOUD_PREFIX in _runner_text(job) and _reachable_by_a_fork(filename)
    ]


def test_the_estate_has_paid_lanes_a_fork_can_reach() -> None:
    """The rule below is over a set, and an empty set proves nothing.

    Scenario: the workflows are searched for Ubicloud lanes on a
    pull-request trigger.

    Invariant: at least one is found. Without this the parametrized
    contract would pass on a repository that had moved every lane back to
    a hosted runner, or on one whose workflow directory had stopped being
    read, and report the fork rule as held.
    """
    assert _paid_lanes(), (
        "no Ubicloud lane on a pull-request trigger was found, so the fork "
        "fallback contract would be asserting over nothing"
    )


@pytest.mark.parametrize(
    ("filename", "job_id", "runner"),
    [pytest.param(*lane, id=f"{lane[0]}:{lane[1]}") for lane in _paid_lanes()],
)
def test_a_paid_lane_declares_its_runner_on_one_line(
    filename: str, job_id: str, runner: str
) -> None:
    """A folded scalar can keep the break the author meant to fold away.

    Scenario: each Ubicloud job's `runs-on` as the YAML parser returns
    it, before any whitespace is normalized.

    Invariant: the value holds no line break. `>-` folds a continuation
    into a space only while the continuation is indented no deeper than
    the line it continues; indent it one level further and YAML treats it
    as a more-indented block and keeps the newline, putting one inside
    the expression. GitHub evaluates the broken value regardless, so a
    green run is not evidence and nothing else here would notice: the
    check below normalizes whitespace, which folds the break away exactly
    as the author intended and exactly as YAML did not.
    """
    assert "\n" not in runner, (
        f"{filename} job {job_id!r} declares its runner across a line break: "
        f"{runner!r}. Keep a folded scalar's continuation at the same indent "
        f"as the line it continues, or YAML keeps the break inside the "
        f"expression"
    )


@pytest.mark.parametrize(
    ("filename", "job_id", "runner"),
    [pytest.param(*lane, id=f"{lane[0]}:{lane[1]}") for lane in _paid_lanes()],
)
def test_a_paid_lane_a_fork_can_reach_falls_back_to_a_hosted_runner(
    filename: str, job_id: str, runner: str
) -> None:
    """A fork gets a hosted runner and everything else gets the paid one.

    Scenario: each Ubicloud job in a workflow a fork's pull request can
    trigger.

    Invariant: its `runs-on` is the event-keyed expression, with the fork
    field as the condition, the hosted runner as the fork arm, and the
    Ubicloud label as the other. All three are read out of the expression
    rather than searched for within it: an expression naming the fork
    field and then ignoring it would satisfy a substring test, and so
    would one with its two arms the wrong way round, which is the shape
    that sends every trusted pull request to a hosted runner and every
    fork to a runner it cannot have.
    """
    parsed = _FALLBACK.fullmatch(" ".join(runner.split()))
    assert parsed is not None, (
        f"{filename} job {job_id!r} names an Ubicloud runner as {runner!r}; a "
        f"fork's pull request cannot obtain one, so the label must be the "
        f"event-keyed expression"
    )
    assert parsed["condition"] == FORK_FIELD, (
        f"{filename} job {job_id!r} keys its runner on "
        f"{parsed['condition']!r}; only {FORK_FIELD} distinguishes a fork's "
        f"pull request from every other event"
    )
    assert parsed["fork"] == FORK_RUNNER, (
        f"{filename} job {job_id!r} sends a fork to {parsed['fork']!r}, which "
        f"is not the hosted runner a fork can actually obtain"
    )
    assert parsed["trusted"].startswith(UBICLOUD_PREFIX), (
        f"{filename} job {job_id!r} sends its trusted events to "
        f"{parsed['trusted']!r}, so the fallback has replaced the paid lane "
        f"rather than guarding it"
    )
