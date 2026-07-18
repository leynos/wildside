"""Contract tests for pull-request coverage enforcement in CI."""

from __future__ import annotations

from pathlib import Path
from typing import cast

import yaml

WORKFLOW_PATH = Path(__file__).resolve().parents[2] / ".github" / "workflows" / "ci.yml"


def _load_steps() -> list[dict[str, object]]:
    """Parse and return the CI coverage steps."""
    workflow = yaml.safe_load(WORKFLOW_PATH.read_text(encoding="utf-8"))
    jobs = workflow.get("jobs")
    assert isinstance(jobs, dict), "the CI workflow must declare jobs"
    coverage = jobs.get("coverage")
    assert isinstance(coverage, dict), "the CI workflow must declare coverage"
    steps = coverage.get("steps")
    assert isinstance(steps, list), "the coverage job must declare steps"
    assert all(isinstance(step, dict) for step in steps), (
        "every coverage step must be a mapping"
    )
    return cast("list[dict[str, object]]", steps)


def _find_step(steps: list[dict[str, object]], name: str) -> dict[str, object]:
    """Return the uniquely named workflow step."""
    matches = [step for step in steps if step.get("name") == name]
    assert len(matches) == 1, f"expected one {name!r} step, found {len(matches)}"
    return matches[0]


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
        "18bed1ca49a6de3d8882bd72635a32ae3f023d57"
    ), "the CodeScene check must use the reviewed shared-action pin"
    assert check.get("with") == {
        "format": "lcov",
        "mode": "check",
        "project-url": "https://api.codescene.io/v2/projects/70675",
        "access-token": "${{ env.CS_ACCESS_TOKEN }}",
        "installer-checksum": "${{ vars.CODESCENE_CLI_SHA256 }}",
    }, "the CodeScene check must pass the canonical project and check-mode inputs"
