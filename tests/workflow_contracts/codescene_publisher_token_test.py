"""The CodeScene token reaches the publisher's upload and nothing else.

The upload is `upload-codescene-coverage`, a composite action, and a composite
action's nested steps inherit the calling step's environment. So a token placed
in the upload step's `env`, or the job's, or the workflow's, reaches every step
inside the action, and every step after it in the job. The publisher therefore
keeps the token out of every `env`. A check step publishes only whether the
token exists, the upload's condition reads that output, and the upload takes
the token directly as an input.

The cases below hold that shape. The positive half matters as much as the
prohibition: a token deleted from the job entirely satisfies "no `env` holds
it", while the upload's non-empty guard goes false and publishing silently
stops. So the token must appear exactly twice in the publisher, in the check
step's command and in the upload's `access-token`.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import typing as typ

import workflow_inventory as inventory
from codescene_baseline import (
    AVAILABILITY_COMMAND,
    AVAILABILITY_STEP_ID,
    CODESCENE_ACTION_MARKER,
    CODESCENE_CREDENTIAL_NAME,
    PUBLISHER,
    UPLOAD_CREDENTIAL_INPUT,
)
from document_strings import strings_in

if typ.TYPE_CHECKING:  # pragma: no cover - annotations only.
    import collections.abc as cabc


def _publisher_steps() -> list[dict[str, typ.Any]]:
    """Return every step of every job in the publisher, in order."""
    return [
        step
        for _, job in inventory.workflow_jobs(PUBLISHER)
        for step in inventory.job_steps(job)
    ]


def _upload_job_steps() -> list[dict[str, typ.Any]]:
    """Return the steps of the one publisher job that uploads to CodeScene.

    The upload reads `steps.<id>.outputs.available`, which resolves only
    inside the job that ran the check. A check in any other job leaves the
    output unset and the upload skipping silently, so the check is looked for
    in the upload's own job and nowhere else.
    """
    jobs = [
        steps
        for steps in (
            inventory.job_steps(job) for _, job in inventory.workflow_jobs(PUBLISHER)
        )
        if any(
            CODESCENE_ACTION_MARKER in inventory.step_action(step).lower()
            for step in steps
        )
    ]
    assert len(jobs) == 1, (
        f"{PUBLISHER} must upload from exactly one job, found {len(jobs)}"
    )
    return jobs[0]


def _token_check(steps: cabc.Sequence[dict[str, typ.Any]]) -> dict[str, typ.Any]:
    """Return the one step carrying the token-check id."""
    checks = [step for step in steps if step.get("id") == AVAILABILITY_STEP_ID]
    assert len(checks) == 1, (
        f"{PUBLISHER} must declare exactly one step with id "
        f"{AVAILABILITY_STEP_ID!r}, found {len(checks)}"
    )
    return checks[0]


def _upload(steps: cabc.Sequence[dict[str, typ.Any]]) -> dict[str, typ.Any]:
    """Return the one CodeScene upload step."""
    uploads = [
        step
        for step in steps
        if CODESCENE_ACTION_MARKER in inventory.step_action(step).lower()
    ]
    assert len(uploads) == 1, f"{PUBLISHER} must upload exactly once"
    return uploads[0]


def test_the_check_step_publishes_availability_and_nothing_else() -> None:
    """Run one command that writes a boolean, unconditionally, with no env.

    A condition on the check would leave its output unset whenever the
    condition was false, and the upload would read that as unavailable. An
    `env` on it would put the token back into an environment.
    """
    check = _token_check(_upload_job_steps())
    assert str(check.get("run", "")).strip() == AVAILABILITY_COMMAND, (
        f"the check step must run exactly {AVAILABILITY_COMMAND!r}, got "
        f"{check.get('run')!r}"
    )
    assert "if" not in check, f"the check step must run unconditionally: {check!r}"
    assert "env" not in check, f"the check step must declare no env: {check!r}"


def test_the_check_precedes_the_upload() -> None:
    """Publish the availability in the upload's job, before the upload."""
    steps = _upload_job_steps()
    assert steps.index(_token_check(steps)) < steps.index(_upload(steps)), (
        "the token check must run before the upload that reads its output"
    )


def test_the_upload_takes_the_token_directly() -> None:
    """Pass the secret as the action's input, not through an environment."""
    upload = _upload(_publisher_steps())
    inputs = upload.get("with")
    assert isinstance(inputs, dict), "the upload must declare its inputs"
    assert inputs.get("access-token") == UPLOAD_CREDENTIAL_INPUT, (
        f"the upload's access-token must be {UPLOAD_CREDENTIAL_INPUT!r}, got "
        f"{inputs.get('access-token')!r}"
    )


def test_no_environment_on_the_publisher_holds_the_token() -> None:
    """Keep the token out of the workflow, job and step environments."""
    document = inventory.load_workflow(PUBLISHER)
    scopes: list[tuple[str, object]] = [("workflow", document.get("env"))]
    scopes += [
        (f"job {job_id}", job.get("env"))
        for job_id, job in inventory.workflow_jobs(PUBLISHER)
    ]
    scopes += [
        (f"step {step.get('name', step.get('id'))}", step.get("env"))
        for step in _publisher_steps()
    ]
    holders = [
        scope
        for scope, env in scopes
        if any(CODESCENE_CREDENTIAL_NAME in text for text in strings_in(env))
    ]
    assert holders == [], (
        f"{PUBLISHER} must keep {CODESCENE_CREDENTIAL_NAME} out of every env; "
        f"found it in {holders}. A composite action's nested steps inherit "
        "the calling step's environment"
    )


def test_the_token_appears_exactly_where_it_is_used() -> None:
    """Name the token in the check's command and the upload's input, only.

    This is the positive half. Deleting the token from the job would satisfy
    every prohibition above while the upload's guard went false and
    publishing stopped without a failure.
    """
    mentions = [
        text
        for text in strings_in(inventory.load_workflow(PUBLISHER))
        if CODESCENE_CREDENTIAL_NAME in text
    ]
    expected = [AVAILABILITY_COMMAND, UPLOAD_CREDENTIAL_INPUT]
    assert sorted(text.strip() for text in mentions) == sorted(expected), (
        f"{PUBLISHER} must name {CODESCENE_CREDENTIAL_NAME} exactly in the "
        f"check command and the upload input, got {mentions}"
    )
