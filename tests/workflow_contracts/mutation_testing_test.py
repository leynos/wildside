"""Contract tests for the mutation-testing caller workflow.

The executable logic lives in the ``leynos/shared-actions`` reusable
workflow, which carries its own unit and integration tests; wildside's
caller is declarative configuration. These tests parse the caller with
PyYAML and assert the caller references the correct reusable workflow
at a commit SHA, so drift (repointing the pin at a branch, widening
permissions, or losing the workspace-wide test configuration) fails CI
on the pull request rather than surfacing in a scheduled or manual
run. Dependabot owns the pinned SHA value; these tests only assert its
shape, so a routine Dependabot bump does not fail CI.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
from pathlib import Path

import yaml

WORKFLOW_PATH = (
    Path(__file__).resolve().parents[2]
    / ".github"
    / "workflows"
    / "mutation-testing.yml"
)

#: Matches the reusable workflow path pinned to a full 40-hex commit
#: SHA. Dependabot owns the SHA value, so it is intentionally not
#: pinned here.
USES_RE = re.compile(
    r"^leynos/shared-actions/\.github/workflows/mutation-cargo\.yml@[0-9a-f]{40}$"
)

#: The exact caller configuration: workspace source prefixes (the
#: vendored third_party/shellexpand patch crate stays out of scope),
#: feature-gated test-support scaffolding excluded, workspace-wide
#: tests with all features to mirror the CI baseline, and the embedded
#: PostgreSQL version pin so the baseline skips postgresql_archive's
#: rate-limited release-listing query (keep aligned with ci.yml's
#: PG_VERSION).
EXPECTED_WITH = {
    "paths": "backend/,crates/,tools/",
    "exclude-globs": (
        "backend/src/test_support.rs,"
        "backend/src/test_support/**,"
        "backend/src/outbound/cache/test_helpers.rs,"
        "backend/src/outbound/queue/test_helpers.rs"
    ),
    "extra-args": "--all-features --test-workspace=true",
    "setup-commands": (
        'echo "POSTGRESQL_VERSION==16.10.0" >> "$GITHUB_ENV"\n'
        'echo "POSTGRESQL_RELEASES_URL='
        'https://github.com/theseus-rs/postgresql-binaries" >> "$GITHUB_ENV"\n'
    ),
}


def _load() -> dict[str, object]:
    """Parse the workflow file."""
    return yaml.safe_load(WORKFLOW_PATH.read_text(encoding="utf-8"))


def _triggers(workflow: dict[str, object]) -> dict[str, object]:
    """Return the ``on:`` mapping (PyYAML parses the bare key as True)."""
    triggers = workflow.get("on", workflow.get(True))
    assert isinstance(triggers, dict), "the workflow must declare an on: mapping"
    return triggers


def _mutation_job(workflow: dict[str, object]) -> dict[str, object]:
    """Return the single calling job."""
    jobs = workflow.get("jobs")
    assert isinstance(jobs, dict), "the workflow must declare a jobs mapping"
    assert jobs, "the workflow must declare at least one job"
    assert list(jobs) == ["mutation"], (
        f"expected a single job named 'mutation', found {sorted(jobs)}"
    )
    return jobs["mutation"]


def test_uses_reference_is_pinned_to_a_commit_sha() -> None:
    """The job must call the correct shared workflow at a commit SHA."""
    uses = _mutation_job(_load()).get("uses")
    assert uses is not None, "jobs.mutation.uses is missing"
    assert USES_RE.match(uses), (
        "jobs.mutation.uses must reference "
        "leynos/shared-actions/.github/workflows/mutation-cargo.yml pinned "
        f"to a full 40-character commit SHA, not a branch or tag: {uses!r}"
    )


def test_job_permissions_are_exactly_least_privilege() -> None:
    """The job grants contents: read and id-token: write, nothing broader."""
    permissions = _mutation_job(_load()).get("permissions")
    assert permissions == {"contents": "read", "id-token": "write"}, (
        "jobs.mutation.permissions must be exactly "
        f"{{'contents': 'read', 'id-token': 'write'}}, got {permissions!r}"
    )


def test_workflow_default_permissions_are_empty() -> None:
    """The workflow-level default token scope is empty."""
    workflow = _load()
    assert workflow.get("permissions") == {}, (
        f"top-level permissions must be an empty mapping, got "
        f"{workflow.get('permissions')!r}"
    )


def test_concurrency_serializes_per_ref_without_cancelling() -> None:
    """Runs queue per ref instead of cancelling one another."""
    concurrency = _load().get("concurrency")
    assert isinstance(concurrency, dict), "the workflow must declare concurrency"
    assert concurrency.get("group") == "mutation-testing-${{ github.ref }}", (
        f"concurrency.group must key on the triggering ref, got "
        f"{concurrency.get('group')!r}"
    )
    assert concurrency.get("cancel-in-progress") is False, (
        f"concurrency.cancel-in-progress must be false, got "
        f"{concurrency.get('cancel-in-progress')!r}"
    )


def test_triggers_keep_schedule_and_plain_dispatch() -> None:
    """The daily schedule stays; dispatch has no legacy branch input."""
    triggers = _triggers(_load())
    schedule = triggers.get("schedule")
    assert schedule == [{"cron": "50 9 * * *"}], (
        f"on.schedule must be the daily 09:50 UTC cron, got {schedule!r}"
    )
    assert "workflow_dispatch" in triggers, "on.workflow_dispatch is missing"
    dispatch = triggers.get("workflow_dispatch") or {}
    inputs = dispatch.get("inputs") or {}
    assert "branch" not in inputs, (
        "on.workflow_dispatch must not declare a branch input; the Actions "
        "run-workflow control selects the ref"
    )


def test_with_block_carries_the_caller_configuration() -> None:
    """The caller passes exactly the documented wildside configuration."""
    with_block = _mutation_job(_load()).get("with")
    assert isinstance(with_block, dict), "jobs.mutation.with is missing"
    assert with_block == EXPECTED_WITH, (
        "jobs.mutation.with must be exactly the documented wildside "
        f"configuration {EXPECTED_WITH!r}, got {with_block!r}"
    )
