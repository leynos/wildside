"""Contract-test Wildside's initial shared Namespace runner assignments."""

from __future__ import annotations

from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]
PROFILE = "namespace-profile-default"


def _job(workflow_name: str, job_name: str) -> dict[str, object]:
    """Load one named job from a repository workflow."""
    workflow_path = ROOT / ".github" / "workflows" / workflow_name
    workflow = yaml.safe_load(workflow_path.read_text(encoding="utf-8"))
    assert isinstance(workflow, dict), f"{workflow_name} must parse to a mapping"
    jobs = workflow.get("jobs")
    assert isinstance(jobs, dict), f"{workflow_name} must declare jobs"
    job = jobs.get(job_name)
    assert isinstance(job, dict), f"{workflow_name} must declare {job_name}"
    return job


def test_low_risk_linux_jobs_use_the_shared_namespace_profile() -> None:
    """Keep the approved utility-job runner assignments from drifting."""
    assert _job("audit.yml", "audit").get("runs-on") == PROFILE
    assert _job("delayed-pr-comment.yml", "delay_and_comment").get("runs-on") == PROFILE


def test_eight_core_build_and_coverage_jobs_remain_on_their_current_runner() -> None:
    """Preserve capacity-sensitive jobs until an equivalent profile exists."""
    assert _job("ci.yml", "build").get("runs-on") == "ubicloud-standard-8"
    assert _job("ci.yml", "coverage").get("runs-on") == "ubicloud-standard-8"
    assert _job("coverage-main.yml", "coverage-upload").get("runs-on") == (
        "ubicloud-standard-8"
    )
