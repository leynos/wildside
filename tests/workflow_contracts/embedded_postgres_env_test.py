"""Contract tests for runner-composed embedded PostgreSQL settings.

Backend test support no longer writes ``PG_PASSWORD`` or
``POSTGRESQL_RELEASES_URL`` into the process (issue 464): every ambient
environment write was removed in favour of injected seams. The embedded
PostgreSQL layer still reads both variables for itself, so the *runner* has to
supply them. Locally that is the ``test-rust`` Make recipe, which
``backend/tests/environment_policy_contract.rs`` guards. In CI it is the
``env:`` block of each step that executes the Rust suite, which these tests
guard.

Mutation proof (2026-09-06): deleting the ``PG_PASSWORD`` line from the CI
``Rust tests`` step failed the ``ci-rust-tests`` case with ``the Rust tests
step must set PG_PASSWORD to the stable default``; the line was restored and
all three cases passed. The ``POSTGRESQL_RELEASES_URL`` assertion caught a real
gap when it was written: the ``Rust tests`` step pinned the release URL only in
the cache warm-up step, so the test run itself was unpinned.
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


def _load_steps(workflow: Path, job_name: str) -> list[dict[str, object]]:
    """Parse and return the steps for one workflow job."""
    document = yaml.safe_load(workflow.read_text(encoding="utf-8"))
    jobs = document.get("jobs")
    assert isinstance(jobs, dict), f"{workflow.name} must declare jobs"
    job = jobs.get(job_name)
    assert isinstance(job, dict), f"{workflow.name} must declare {job_name}"
    steps = job.get("steps")
    assert isinstance(steps, list), f"{job_name} must declare steps"
    return typ.cast("list[dict[str, object]]", steps)


def _step_env(steps: list[dict[str, object]], step_name: str) -> dict[str, object]:
    """Return the ``env`` mapping of the named step."""
    matches = [step for step in steps if step.get("name") == step_name]
    assert len(matches) == 1, f"expected exactly one {step_name!r} step"
    env = matches[0].get("env")
    assert isinstance(env, dict), f"the {step_name!r} step must declare env"
    return typ.cast("dict[str, object]", env)


@pytest.mark.parametrize(
    ("workflow_name", "job_name", "step_name"),
    [
        pytest.param("ci.yml", "build", "Rust tests", id="ci-rust-tests"),
        pytest.param("ci.yml", "coverage", "Generate Rust coverage", id="ci-coverage"),
        pytest.param(
            "coverage-main.yml",
            "coverage-upload",
            "Generate Rust coverage",
            id="coverage-main",
        ),
    ],
)
def test_ci_rust_tests_step_composes_embedded_postgres_settings(
    workflow_name: str, job_name: str, step_name: str
) -> None:
    """Every step running the Rust suite supplies the embedded PG settings.

    Test support resolves the password without setting it, so a step that
    omits it would let ``postgresql_embedded`` generate a random one and fail
    later test binaries with ``28P01 password authentication failed``.
    """
    env = _step_env(_load_steps(WORKFLOWS / workflow_name, job_name), step_name)

    assert env.get("PG_PASSWORD") == STABLE_PG_PASSWORD, (
        f"the {step_name} step must set PG_PASSWORD to the stable default"
    )
    assert env.get("POSTGRESQL_RELEASES_URL") == THESEUS_RELEASES_URL, (
        f"the {step_name} step must pin POSTGRESQL_RELEASES_URL"
    )
