"""Contract tests for runner-composed embedded PostgreSQL settings.

Backend test support no longer writes ``PG_PASSWORD`` or
``POSTGRESQL_RELEASES_URL`` into the process (issue 464): every ambient
environment write was removed in favour of injected seams. The embedded
PostgreSQL layer still reads both variables for itself, so the *runner* has to
supply them. Locally that is the ``test-rust`` Make recipe, which
``backend/tests/environment_policy_contract.rs`` guards. In CI it is the
``env:`` block of each step that executes the Rust suite, which these tests
guard.

A step's ``env`` block only matters if the step runs, so each case also
asserts that the lane is reachable: the workflow declares the trigger the lane
depends on, the step carries no condition at all, and the job carries either no
condition or exactly the reviewed one. Without that, ``if: false`` on the step
or the job would skip the suite while leaving every ``env`` value in place, and
this contract would certify a lane that never executes.

Mutation proofs, each reverted afterwards:

- 2026-09-06, deleting the ``PG_PASSWORD`` line from the CI ``Rust tests``
  step failed the ``ci-rust-tests`` case with ``the Rust tests step must set
  PG_PASSWORD to the stable default``.
- 2026-09-07, ``if: false`` on the ``Rust tests`` step, ``if: false`` on the
  ``build`` job, replacing the ``coverage`` job's condition with a push-only
  one, and deleting ``pull_request`` from ``ci.yml``'s triggers each failed
  their case.

The ``POSTGRESQL_RELEASES_URL`` assertion caught a real gap when it was
written: the ``Rust tests`` step pinned the release URL only in the cache
warm-up step, so the test run itself was unpinned.
"""

from __future__ import annotations

import typing as typ
from pathlib import Path

import pytest
import yaml

WORKFLOWS = Path(__file__).resolve().parents[2] / ".github" / "workflows"

# Not a secret: this is the documented default superuser password for the
# throwaway embedded test cluster, and it is declared in the Makefile and both
# workflows in plain text for exactly that reason.
STABLE_PG_PASSWORD = "wildside_embedded_test"  # noqa: S105
THESEUS_RELEASES_URL = "https://github.com/theseus-rs/postgresql-binaries"

# The `coverage` job runs on pull requests only, and not for Dependabot. That
# condition is pinned verbatim rather than merely required to be absent, so
# narrowing it to `false` or to a push-only event fails this contract.
COVERAGE_JOB_CONDITION = (
    "github.actor != 'dependabot[bot]' && github.event_name != 'push'"
)


def _load(workflow: Path) -> dict[str, object]:
    """Parse a workflow file."""
    document = yaml.safe_load(workflow.read_text(encoding="utf-8"))
    assert isinstance(document, dict), f"{workflow.name} must parse as a mapping"
    return typ.cast("dict[str, object]", document)


def _triggers(document: dict[str, object]) -> dict[str, object]:
    """Return a workflow's ``on`` mapping.

    YAML 1.1 parses the bare key ``on`` as the boolean ``True``, so the
    document is keyed on either spelling depending on how it was written.
    """
    raw = document.get(True, document.get("on"))
    assert isinstance(raw, dict), "the workflow must declare its triggers as a mapping"
    return typ.cast("dict[str, object]", raw)


def _job(document: dict[str, object], job_name: str) -> dict[str, object]:
    """Return one job definition."""
    jobs = document.get("jobs")
    assert isinstance(jobs, dict), "the workflow must declare jobs"
    job = jobs.get(job_name)
    assert isinstance(job, dict), f"the workflow must declare the {job_name} job"
    return typ.cast("dict[str, object]", job)


def _step(job: dict[str, object], step_name: str) -> dict[str, object]:
    """Return the single step with the given name."""
    steps = job.get("steps")
    assert isinstance(steps, list), "the job must declare steps"
    matches = [
        step
        for step in steps
        if isinstance(step, dict) and step.get("name") == step_name
    ]
    assert len(matches) == 1, f"expected exactly one {step_name!r} step"
    return typ.cast("dict[str, object]", matches[0])


@pytest.mark.parametrize(
    ("workflow_name", "job_name", "step_name", "required_trigger", "job_condition"),
    [
        pytest.param(
            "ci.yml", "build", "Rust tests", "pull_request", None, id="ci-rust-tests"
        ),
        pytest.param(
            "ci.yml",
            "coverage",
            "Generate Rust coverage",
            "pull_request",
            COVERAGE_JOB_CONDITION,
            id="ci-coverage",
        ),
        pytest.param(
            "coverage-main.yml",
            "coverage-upload",
            "Generate Rust coverage",
            "push",
            None,
            id="coverage-main",
        ),
    ],
)
def test_the_rust_suite_receives_the_embedded_postgres_settings(
    workflow_name: str,
    job_name: str,
    step_name: str,
    required_trigger: str,
    job_condition: str | None,
) -> None:
    """Each step running the Rust suite is reachable and supplies the settings.

    Test support resolves the password without setting it, so a step that omits
    it would let ``postgresql_embedded`` generate a random one and fail later
    test binaries with ``28P01 password authentication failed``. A step that is
    skipped supplies nothing at all, so reachability is asserted first.
    """
    document = _load(WORKFLOWS / workflow_name)

    assert required_trigger in _triggers(document), (
        f"{workflow_name} must trigger on {required_trigger}, or the "
        f"{step_name} step never runs"
    )

    job = _job(document, job_name)
    assert job.get("if") == job_condition, (
        f"the {job_name} job's condition must be {job_condition!r}; a changed "
        f"or added condition can skip {step_name} entirely"
    )

    step = _step(job, step_name)
    assert "if" not in step, (
        f"the {step_name} step must carry no condition; one would let it be "
        "skipped while its env block still reads correctly"
    )

    env = step.get("env")
    assert isinstance(env, dict), f"the {step_name!r} step must declare env"
    assert env.get("PG_PASSWORD") == STABLE_PG_PASSWORD, (
        f"the {step_name} step must set PG_PASSWORD to the stable default"
    )
    assert env.get("POSTGRESQL_RELEASES_URL") == THESEUS_RELEASES_URL, (
        f"the {step_name} step must pin POSTGRESQL_RELEASES_URL"
    )
