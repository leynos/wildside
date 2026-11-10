"""Contract tests for where the split main-branch coverage jobs run.

Main-branch coverage is three jobs. The measurement runs on a GitHub-hosted
runner, where compute is free; the baseline publisher and the cache warm-up run
on the managed runners, whose cache store the pull-request lanes read.
"""

from __future__ import annotations

import runner_shapes
import workflow_inventory as inv


def test_the_hosted_coverage_job_is_github_hosted_with_a_timeout() -> None:
    """Main-branch coverage is measured on a GitHub-hosted runner, bounded.

    Hosted compute is free on a public repository, so the measurement does not
    take a managed runner. A hung step must still stop before the platform's
    six-hour cap.
    """
    filename, job_id = inv.HOSTED_COVERAGE_JOB
    job = inv.load_workflow(filename)["jobs"][job_id]
    labels = runner_shapes.runner_labels(job)
    assert labels, f"{filename}:{job_id} must declare a runner label"
    assert labels <= inv.GITHUB_HOSTED_LABELS, (
        f"{filename}:{job_id} must run on a GitHub-hosted runner, and selects "
        f"{sorted(labels)}"
    )
    assert isinstance(job.get("timeout-minutes"), int), (
        f"{filename}:{job_id} must declare an integer timeout-minutes"
    )


def test_the_baseline_job_takes_the_smallest_managed_runner() -> None:
    """The baseline publisher compiles nothing and reads the managed cache store."""
    filename, job_id = inv.BASELINE_JOB
    job = inv.load_workflow(filename)["jobs"][job_id]
    assert runner_shapes.runner_labels(job) == {"ubicloud-standard-2"}, (
        f"{filename}:{job_id} must run on ubicloud-standard-2"
    )
    assert isinstance(job.get("timeout-minutes"), int), (
        f"{filename}:{job_id} must declare an integer timeout-minutes"
    )
