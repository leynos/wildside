"""Contract tests for main-branch Rust coverage uploads."""

from __future__ import annotations

import typing as typ
from pathlib import Path

import yaml

WORKFLOW_PATH = (
    Path(__file__).resolve().parents[2] / ".github" / "workflows" / "coverage-main.yml"
)
COVERAGE_FILE = "lcov.info"
GENERATE_COVERAGE_PIN = (
    "leynos/shared-actions/.github/actions/generate-coverage@"
    "074f7d8ba75a6e5d18532b72cbe38fccbda4e9c6"
)
UPLOAD_CODESCENE_PIN = (
    "leynos/shared-actions/.github/actions/upload-codescene-coverage@"
    "18bed1ca49a6de3d8882bd72635a32ae3f023d57"
)


def _load_steps(job_name: str = "coverage-upload") -> list[dict[str, object]]:
    """Parse and return the steps for one workflow job."""
    workflow = yaml.safe_load(WORKFLOW_PATH.read_text(encoding="utf-8"))
    jobs = workflow.get("jobs")
    assert isinstance(jobs, dict), "the coverage workflow must declare jobs"
    job = jobs.get(job_name)
    assert isinstance(job, dict), f"the coverage workflow must declare {job_name}"
    steps = job.get("steps")
    assert isinstance(steps, list), f"the {job_name} job must declare steps"
    assert all(isinstance(step, dict) for step in steps), (
        "every coverage step must be a mapping"
    )
    return typ.cast("list[dict[str, object]]", steps)


def _find_step(steps: list[dict[str, object]], name: str) -> dict[str, object]:
    """Return the uniquely named workflow step."""
    matches = [step for step in steps if step.get("name") == name]
    assert len(matches) == 1, f"expected one {name!r} step, found {len(matches)}"
    return matches[0]


def test_coverage_generation_uses_the_reviewed_action_pin() -> None:
    """Main-branch coverage runs the immutably pinned shared action."""
    generation = _find_step(_load_steps(), "Generate Rust coverage")

    assert generation.get("uses") == GENERATE_COVERAGE_PIN, (
        "coverage generation must stay on the reviewed immutable"
        " generate-coverage commit"
    )


def test_coverage_generation_forces_rust_only_lcov() -> None:
    """The root pyproject.toml is tooling-only, so coverage stays Rust-only.

    Without ``language: rust`` the action's auto-detection reads the root
    ``pyproject.toml`` as a Python project, classifies the repository as mixed,
    and rejects ``lcov`` — which would break the report CodeScene consumes.
    """
    generation = _find_step(_load_steps(), "Generate Rust coverage")

    assert generation.get("with") == {
        "language": "rust",
        "output-path": COVERAGE_FILE,
        "format": "lcov",
        "use-cargo-nextest": "true",
        "features": "example-data metrics test-support",
        "with-ratchet": "true",
    }, "main-branch coverage must force Rust-only ratcheted LCOV output"


def test_codescene_upload_consumes_the_generated_lcov_report() -> None:
    """The upload step publishes the LCOV report the previous step wrote."""
    steps = _load_steps()
    generation = _find_step(steps, "Generate Rust coverage")
    upload = _find_step(steps, "Upload coverage data to CodeScene")

    assert steps.index(upload) > steps.index(generation), (
        "the CodeScene upload must follow coverage generation"
    )
    generation_options = generation.get("with")
    assert isinstance(generation_options, dict), "coverage generation needs inputs"
    assert generation_options.get("output-path") == COVERAGE_FILE, (
        "coverage generation must write the report the upload step consumes"
    )
    assert upload.get("uses") == UPLOAD_CODESCENE_PIN, (
        "the CodeScene upload must stay on the reviewed immutable"
        " upload-codescene-coverage commit"
    )
    upload_options = upload.get("with")
    assert isinstance(upload_options, dict), "the CodeScene upload needs inputs"
    assert upload_options.get("format") == "lcov", (
        "the CodeScene upload must read the generated report as LCOV"
    )
