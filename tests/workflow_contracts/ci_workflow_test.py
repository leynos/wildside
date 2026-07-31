"""Contract tests for pull-request quality enforcement in CI."""

from __future__ import annotations

from pathlib import Path
from typing import cast

import yaml

WORKFLOW_PATH = Path(__file__).resolve().parents[2] / ".github" / "workflows" / "ci.yml"


def _load_steps(job_name: str = "coverage") -> list[dict[str, object]]:
    """Parse and return the steps for a named CI job."""
    workflow = yaml.safe_load(WORKFLOW_PATH.read_text(encoding="utf-8"))
    jobs = workflow.get("jobs")
    assert isinstance(jobs, dict), "the CI workflow must declare jobs"
    job = jobs.get(job_name)
    assert isinstance(job, dict), f"the CI workflow must declare {job_name}"
    steps = job.get("steps")
    assert isinstance(steps, list), f"the {job_name} job must declare steps"
    assert all(isinstance(step, dict) for step in steps), (
        "every CI step must be a mapping"
    )
    return cast("list[dict[str, object]]", steps)


def _find_step(steps: list[dict[str, object]], name: str) -> dict[str, object]:
    """Return the uniquely named workflow step."""
    matches = [step for step in steps if step.get("name") == name]
    assert len(matches) == 1, f"expected one {name!r} step, found {len(matches)}"
    return matches[0]


def test_build_runs_the_typedoc_documentation_gate() -> None:
    """Pull requests must reject undocumented JavaScript and TypeScript APIs."""
    documentation = _find_step(
        _load_steps("build"),
        "TypeDoc documentation gate",
    )
    assert documentation.get("run") == "make docs-check"


def test_codescene_check_immediately_follows_coverage_generation() -> None:
    """The changed-line gate consumes the LCOV report produced just before it."""
    steps = _load_steps()
    generation = _find_step(steps, "Generate Rust coverage")
    check = _find_step(steps, "Check coverage against CodeScene gates")
    assert steps.index(check) == steps.index(generation) + 1, (
        "the CodeScene check must immediately follow coverage generation"
    )
    assert generation.get("with") == {
        "output-path": "lcov.info",
        "format": "lcov",
        "use-cargo-nextest": "true",
        "features": "example-data metrics test-support",
        "with-ratchet": "true",
    }, "coverage generation must preserve Wildside's ratcheted LCOV mapping"


def test_codescene_check_uses_the_guarded_project_contract() -> None:
    """The CodeScene check is fork-safe and targets Wildside's project."""
    check = _find_step(_load_steps(), "Check coverage against CodeScene gates")
    assert check.get("env") == {"CS_ACCESS_TOKEN": "${{ secrets.CS_ACCESS_TOKEN }}"}, (
        "the CodeScene token must remain scoped to the check step"
    )
    assert check.get("if") == (
        "github.event_name == 'pull_request' && env.CS_ACCESS_TOKEN != ''"
    ), "the CodeScene check must skip pull requests without the secret"
    assert check.get("uses") == (
        "leynos/shared-actions/.github/actions/upload-codescene-coverage@"
        "927edd45ae77be4251a8a18ca9eb5613a2e32cbd"
    ), "the CodeScene check must use the reviewed shared-action pin"
    assert check.get("with") == {
        "format": "lcov",
        "mode": "check",
        "project-url": "https://api.codescene.io/v2/projects/70675",
        "access-token": "${{ env.CS_ACCESS_TOKEN }}",
        "installer-checksum": "${{ vars.CODESCENE_CLI_SHA256 }}",
    }, "the CodeScene check must pass the canonical project and check-mode inputs"
