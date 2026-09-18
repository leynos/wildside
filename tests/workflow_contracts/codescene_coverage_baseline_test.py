"""Contracts holding the CodeScene coverage baseline, CV-005.

A pull-request lane used to run CodeScene's changed-line gate: it called
`upload-codescene-coverage` with `mode: check`, took `CS_ACCESS_TOKEN`
from secrets, and fetched the full history so `cs-coverage check` could
diff against the merge base. The gate was real, but its cost was paid on
every pull request and it is not the only way to get the protection.

Two things follow from putting it on a pull request, and both are why the
baseline moves it off:

- **A CLI break stops every branch at once.** The CodeScene CLI was
  unpinned in the shared action, and on 2026-09-16 a floating version
  broke Cobertura parsing and reddened every branch in several
  repositories on the same day. A gate on the trunk lane fails one
  workflow run; the same gate on the pull-request lane fails all of
  them, and no branch can merge its way out.
- **A fork cannot hold the secret.** The step carried
  `env.CS_ACCESS_TOKEN != ''` precisely so a forked pull request could
  skip it, which means the gate was never enforced for the contributors
  most likely to need it. A guard that silently does nothing for a whole
  class of pull request is not a gate.

The replacement is not "no coverage gate". `generate-coverage` runs on
both lanes with `with-ratchet: true`, so a pull request that lowers
coverage fails on the ratchet without any CodeScene round trip, and
`coverage-main.yml` uploads on a push to `main` and advances the
baseline that pull-request runs ratchet against. CodeScene still sees
this repository's coverage; it sees it from the trunk.

So the shape asserted here is: no CodeScene action, no `cs-coverage`
command and no `CS_ACCESS_TOKEN` on any workflow that runs on a pull
request; one push-to-main workflow that uploads; and the pull-request
lane ratcheting against the same baseline the publisher advances.

Each assertion is proved by putting the element it forbids back. A
contract that named the secret and the action without being driven by
their return would be satisfied by a workflow that spelled either
differently.
"""

from __future__ import annotations

import typing as typ

import pytest
import workflow_inventory as inventory

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: The *name* of the variable the CodeScene CLI authenticates with. This
#: holds the identifier, never a value: the contract searches for the
#: name because both the `env:` key and the `secrets.` reference spell it
#: identically, and either one puts the credential on the lane.
CODESCENE_CREDENTIAL_NAME = "CS_ACCESS_TOKEN"

#: Any action under a path naming CodeScene. Matched on the path rather
#: than on a pin, so a repin does not silently reintroduce the step.
CODESCENE_ACTION_MARKER = "codescene"

#: The CLI, in both spellings: the standalone `cs-coverage` binary the
#: shared action installs, and the `cs` binary's `coverage` subcommand.
COVERAGE_CLI = "cs-coverage"
CLI_BINARY = "cs"
CLI_SUBCOMMAND = "coverage"

#: The workflow allowed to talk to CodeScene, and the event it does it on.
PUBLISHER = "coverage-main.yml"
PUBLISHER_EVENT = "push"

#: The action that produces coverage on both lanes.
GENERATE_COVERAGE = "generate-coverage"

#: The inputs that decide which coverage number is produced. The ratchet
#: compares a pull request against a baseline the publisher wrote, so a
#: lane disagreeing with the publisher on any of these is comparing two
#: different measurements and the comparison means nothing.
BASELINE_INPUTS = ("language", "format", "output-path", "use-cargo-nextest", "features")


def _strings(node: object) -> cabc.Iterator[str]:
    """Yield every string in a parsed document, keys and values alike.

    A secret reaches a workflow as an `env:` key, as a `${{ secrets.X }}`
    value, or as an action input, and a contract that walked only one of
    those would miss the others. YAML comments never reach here, so prose
    about a banned form is not mistaken for the form.

    Parameters
    ----------
    node : object
        Any part of a parsed workflow.

    Yields
    ------
    str
        Each string found, at any depth.
    """
    if isinstance(node, str):
        yield node
    elif isinstance(node, dict):
        yield from _mapping_strings(node)
    elif isinstance(node, list):
        for item in node:
            yield from _strings(item)


def _mapping_strings(mapping: cabc.Mapping[typ.Any, typ.Any]) -> cabc.Iterator[str]:
    """Yield every string in one mapping, its keys included.

    Split out of :func:`_strings` rather than nested inside it. Walking a
    mapping needs both halves of each entry and walking a sequence needs
    neither, and holding both shapes in one body was the nesting
    CodeScene flagged when this module was written.

    Parameters
    ----------
    mapping : collections.abc.Mapping[typing.Any, typing.Any]
        Any mapping from a parsed workflow.

    Yields
    ------
    str
        Each string key, and each string anywhere in each value.
    """
    for key, value in mapping.items():
        if isinstance(key, str):
            yield key
        yield from _strings(value)


def _generate_coverage_inputs(filename: str) -> list[dict[str, object]]:
    """Return the inputs of every `generate-coverage` step in one workflow.

    Parameters
    ----------
    filename : str
        The workflow file's name.

    Returns
    -------
    list[dict[str, object]]
        One mapping per matching step, in file order.
    """
    found: list[dict[str, object]] = []
    for name, job in inventory.workflow_jobs(filename):
        del name
        for step in inventory.job_steps(job):
            if GENERATE_COVERAGE not in inventory.step_action(step):
                continue
            options = step.get("with")
            found.append(options if isinstance(options, dict) else {})
    return found


@pytest.mark.parametrize("filename", inventory.pull_request_workflows())
def test_no_pull_request_workflow_holds_the_codescene_credential(filename: str) -> None:
    """A workflow running on a pull request never sees the CodeScene secret.

    Scenario: every workflow whose triggers include `pull_request` or
    `pull_request_target`. Invariant: `CS_ACCESS_TOKEN` appears nowhere
    in the parsed document, as an `env:` key, a `secrets.` reference or
    an action input.

    The whole document is walked rather than the `env:` blocks alone,
    because the same secret reaches a step three ways and a contract
    checking one of them is satisfied by the other two.
    """
    document = inventory.load_workflow(filename)
    holders = [text for text in _strings(document) if CODESCENE_CREDENTIAL_NAME in text]
    assert holders == [], (
        f"{filename} runs on a pull request and names {CODESCENE_CREDENTIAL_NAME} in "
        f"{holders}; only {PUBLISHER} may hold it, because a gate that a fork "
        "cannot run is not a gate and a CodeScene outage must not stop every "
        "branch at once"
    )


@pytest.mark.parametrize("filename", inventory.pull_request_workflows())
def test_no_pull_request_workflow_uses_a_codescene_action(filename: str) -> None:
    """No pull-request lane calls a CodeScene action.

    Scenario: the same workflows, read for the actions their steps use.
    Invariant: no `uses` value names CodeScene. Matched on the action's
    path rather than on its pin, so bumping the pin cannot reintroduce
    the step under a contract that still passes.
    """
    offenders = [
        f"{job_id}/{step.get('name', inventory.step_action(step))}"
        for job_id, job in inventory.workflow_jobs(filename)
        for step in inventory.job_steps(job)
        if CODESCENE_ACTION_MARKER in inventory.step_action(step).lower()
    ]
    assert offenders == [], (
        f"{filename} runs on a pull request and calls a CodeScene action in "
        f"{offenders}; the coverage gate on this lane is the ratchet, and "
        f"{PUBLISHER} is the only workflow that talks to CodeScene"
    )


@pytest.mark.parametrize("filename", inventory.pull_request_workflows())
def test_no_pull_request_workflow_runs_a_cs_coverage_command(filename: str) -> None:
    """No pull-request lane runs the CodeScene coverage CLI.

    Scenario: every shell command in the workflow, with comments removed
    and continuations joined by the shared reader, so prose about the
    command is not mistaken for the command and a flag on a second line
    is still part of it. Invariant: no command invokes `cs-coverage`, in
    either the standalone spelling or as the `cs` binary's `coverage`
    subcommand.

    This is the route the action assertion cannot see. Dropping the
    action and running the CLI from a `run:` block would pass that
    contract and reintroduce exactly the parse step this baseline
    removes.
    """
    offenders = [
        f"{job_id}/{step_name}: {command}"
        for name, job_id, step_name, command in inventory.iter_step_commands()
        if name == filename and _invokes_the_coverage_cli(command)
    ]
    assert offenders == [], (
        f"{filename} runs on a pull request and invokes the CodeScene "
        f"coverage CLI in {offenders}"
    )


def _invokes_the_coverage_cli(command: str) -> bool:
    """Return whether one command runs the CodeScene coverage CLI.

    Words are compared whole. A substring test would report a path such
    as `scripts/cs-coverage-notes.md` and, worse, would miss nothing it
    needs: the CLI is always spelled as its own word.

    Parameters
    ----------
    command : str
        One logical shell command.

    Returns
    -------
    bool
        True when the command invokes the CLI by either spelling.

    Examples
    --------
    >>> _invokes_the_coverage_cli("cs-coverage check --format lcov")
    True
    >>> _invokes_the_coverage_cli("cs coverage upload")
    True
    >>> _invokes_the_coverage_cli("echo 'see the cs-coverage notes'")
    False
    >>> _invokes_the_coverage_cli("cargo llvm-cov --lcov")
    False
    """
    words = command.split()
    if not words:
        return False
    if words[0] == COVERAGE_CLI:
        return True
    return words[0] == CLI_BINARY and len(words) > 1 and words[1] == CLI_SUBCOMMAND


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
    assert PUBLISHER not in inventory.pull_request_workflows(), (
        f"{PUBLISHER} holds the CodeScene secret, so it must never run on a "
        "pull request"
    )
    uploads = [
        step.get("name")
        for _, job in inventory.workflow_jobs(PUBLISHER)
        for step in inventory.job_steps(job)
        if CODESCENE_ACTION_MARKER in inventory.step_action(step).lower()
    ]
    assert uploads, (
        f"{PUBLISHER} must call a CodeScene action; with no publisher the "
        "ratchet compares every pull request against a baseline that nothing "
        "advances, and CodeScene sees no coverage for this repository at all"
    )


def test_the_pull_request_lane_ratchets_against_the_publishers_baseline() -> None:
    """Both lanes ratchet, and both measure the same thing.

    Scenario: the `generate-coverage` step in the pull-request lane and
    the one in the publisher. Invariant: each sets `with-ratchet` to
    true, and the two agree on every input that decides which number is
    produced.

    The agreement is the point rather than the ratchet flag alone. The
    publisher writes the baseline and the pull-request lane compares
    against it, so a lane selecting different features or a different
    format is ratcheting one measurement against another, and the
    comparison is meaningless while still passing a flag-only contract.
    """
    lane = _generate_coverage_inputs("ci.yml")
    publisher = _generate_coverage_inputs(PUBLISHER)
    assert len(lane) == 1, f"expected one coverage generation in ci.yml, found {lane}"
    assert len(publisher) == 1, (
        f"expected one coverage generation in {PUBLISHER}, found {publisher}"
    )

    for label, options in (("ci.yml", lane[0]), (PUBLISHER, publisher[0])):
        assert options.get("with-ratchet") == "true", (
            f"{label} must generate coverage with with-ratchet true; the "
            "ratchet is what replaced the CodeScene changed-line gate on the "
            "pull-request lane"
        )

    differing = {
        name: (lane[0].get(name), publisher[0].get(name))
        for name in BASELINE_INPUTS
        if lane[0].get(name) != publisher[0].get(name)
    }
    assert differing == {}, (
        f"ci.yml and {PUBLISHER} must measure the same coverage; they differ "
        f"on {differing}, so the pull-request ratchet compares one "
        "measurement against a baseline written from another"
    )
