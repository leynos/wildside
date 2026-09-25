"""Whether the publisher uploads, for each token, event and ref it can meet.

The other publisher contracts pin the check command and the upload's
condition as text. This one runs them. The check step's own script runs
under `bash` with its secret expression rendered as GitHub renders it, and
the output it writes feeds the upload's declared `if:`, which
`github_conditions` evaluates. So the decision is read end to end from the
workflow file: an absent token skips the upload, and so does any ref but
`main`, on a push or a dispatch alike.

Running the workflow itself, under `act` or on a runner, would need the
CodeScene token and the `ubicloud-standard-8` label; the upload's real proof
is the publisher run on the merge commit.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import shutil
import subprocess  # noqa: S404 - the check step's script is under test.
import typing as typ

import github_conditions as conditions
import pytest
import workflow_inventory as inventory
from codescene_baseline import AVAILABILITY_STEP_ID, CODESCENE_ACTION_MARKER, PUBLISHER

if typ.TYPE_CHECKING:
    import collections.abc as cabc
    from pathlib import Path

#: The check's expression, which GitHub renders to `true` or `false` before
#: the shell runs.
AVAILABILITY_EXPRESSION = "${{ secrets.CS_ACCESS_TOKEN != '' }}"
AVAILABILITY_OUTPUT = f"steps.{AVAILABILITY_STEP_ID}.outputs.available"
TRUNK = "refs/heads/main"


def _publisher_step(
    predicate: cabc.Callable[[dict[str, typ.Any]], bool],
) -> dict[str, typ.Any]:
    """Return the one publisher step matching ``predicate``."""
    matches = [
        step
        for _, job in inventory.workflow_jobs(PUBLISHER)
        for step in inventory.job_steps(job)
        if predicate(step)
    ]
    assert len(matches) == 1, f"{PUBLISHER} must hold exactly one such step"
    return matches[0]


def _published_availability(tmp_path: Path, *, has_token: bool) -> str:
    """Run the check step's script and return the `available` it writes."""
    check = _publisher_step(lambda step: step.get("id") == AVAILABILITY_STEP_ID)
    script = str(check.get("run", ""))
    assert AVAILABILITY_EXPRESSION in script, (
        f"the check must render {AVAILABILITY_EXPRESSION!r}, got {script!r}"
    )
    output = tmp_path / "github_output"
    bash = shutil.which("bash")
    assert bash is not None, "bash must be installed to run the check step"
    subprocess.run(  # noqa: S603 - the workflow's own script, rendered here.
        [bash, "-c", script.replace(AVAILABILITY_EXPRESSION, str(has_token).lower())],
        check=True,
        env={"PATH": "/usr/bin:/bin", "GITHUB_OUTPUT": str(output)},
    )
    outputs = dict(line.split("=", 1) for line in output.read_text().splitlines())
    return outputs["available"]


class Scenario(typ.NamedTuple):
    """One run the publisher can meet, and whether it should upload."""

    has_token: bool
    event_name: str
    ref: str
    uploads: bool


BRANCH = "refs/heads/feature"


@pytest.mark.parametrize(
    "scenario",
    [
        pytest.param(
            Scenario(has_token=True, event_name="push", ref=TRUNK, uploads=True),
            id="push-to-main",
        ),
        pytest.param(
            Scenario(
                has_token=True, event_name="workflow_dispatch", ref=TRUNK, uploads=True
            ),
            id="dispatch-main",
        ),
        pytest.param(
            Scenario(has_token=False, event_name="push", ref=TRUNK, uploads=False),
            id="no-token",
        ),
        pytest.param(
            Scenario(
                has_token=False,
                event_name="workflow_dispatch",
                ref=TRUNK,
                uploads=False,
            ),
            id="no-token-dispatch",
        ),
        pytest.param(
            Scenario(
                has_token=True,
                event_name="workflow_dispatch",
                ref=BRANCH,
                uploads=False,
            ),
            id="dispatch-branch",
        ),
        pytest.param(
            Scenario(
                has_token=False,
                event_name="workflow_dispatch",
                ref=BRANCH,
                uploads=False,
            ),
            id="none",
        ),
    ],
)
def test_the_publisher_uploads_only_with_a_token_on_main(
    tmp_path: Path, scenario: Scenario
) -> None:
    """Upload exactly when the token exists and the run is on `main`.

    An absent token must skip rather than fail, and a dispatch from a feature
    branch must never upload that branch's coverage as the trunk's.
    """
    upload = _publisher_step(
        lambda step: CODESCENE_ACTION_MARKER in inventory.step_action(step).lower()
    )
    available = _published_availability(tmp_path, has_token=scenario.has_token)
    context = {
        "event_name": scenario.event_name,
        "ref": scenario.ref,
        AVAILABILITY_OUTPUT: available,
    }
    decided = conditions.evaluate(upload.get("if"), context)
    assert decided is scenario.uploads, (
        f"{scenario}: expected upload={scenario.uploads}, the workflow decides "
        f"{decided}"
    )
