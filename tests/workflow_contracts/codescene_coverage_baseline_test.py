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

import pytest
import shell_invocations as shell
import workflow_inventory as inventory
from codescene_baseline import (
    BASELINE_INPUTS,
    CLI_BINARY,
    CLI_SUBCOMMAND,
    CODESCENE_ACTION_MARKER,
    CODESCENE_CREDENTIAL_NAME,
    COVERAGE_CLI,
    EXPECTED_PULL_REQUEST_WORKFLOWS,
    GENERATE_COVERAGE,
    PUBLISHER,
)
from document_strings import strings_in


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


def test_every_pull_request_workflow_is_discovered() -> None:
    """The absence contracts run against every pull-request workflow.

    Scenario: the repository's six workflows, two of which run on a
    contributor's pull request. Invariant: discovery finds exactly those
    two.

    This exists because of how the discovery can fail. YAML 1.1 parses a
    bare `on` key as the boolean `True`, and `dependabot-automerge.yml`
    spells it bare while `ci.yml` quotes it. A reader that looked only
    under the string key would return `ci.yml` alone, every absence
    contract below would still pass, and the workflow that runs with the
    base repository's secrets on `pull_request_target` would be the one
    nothing checked. Parametrizing over a discovered list means a
    shrinking list shrinks the suite silently, so the list is pinned.
    """
    assert tuple(inventory.pull_request_workflows()) == (
        EXPECTED_PULL_REQUEST_WORKFLOWS
    ), (
        "discovery must find every workflow triggered by pull_request or "
        f"pull_request_target; expected {list(EXPECTED_PULL_REQUEST_WORKFLOWS)}, "
        f"got {inventory.pull_request_workflows()}"
    )


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
    holders = [
        text for text in strings_in(document) if CODESCENE_CREDENTIAL_NAME in text
    ]
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

    Both spellings are read, the standalone `cs-coverage` binary and the
    `cs` binary's `coverage` subcommand, and each through
    :func:`shell_invocations.invokes`, which resolves the executable past
    assignments, transparent wrappers and a path prefix. Reading the
    first word instead would return False for `env cs-coverage check`,
    and a rule satisfied by a spelling is worked around by choosing it.

    Parameters
    ----------
    command : str
        One logical shell command.

    Returns
    -------
    bool
        True when the command invokes the CLI by either spelling.
    """
    return shell.invokes(command, COVERAGE_CLI) or shell.invokes(
        command, CLI_BINARY, CLI_SUBCOMMAND
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
