"""The placement contracts see a paid lane written in any `runs-on` form.

`runner_labels_test.py` holds the reader. This module holds the contracts
that consume it, because the repository's own workflows spell every runner as
a scalar or an event-keyed expression. A consumer that fell back to reading
the raw value would pass against them while a mapping-form paid lane slipped
through. Each case writes a small workflow tree, points the inventory at it,
and runs the consumer the placement contracts use.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import typing as typ

import pytest
import runner_placement_test as placement
import tool_installation_test as tools
import workflow_inventory as inv

if typ.TYPE_CHECKING:  # pragma: no cover - annotations only.
    from pathlib import Path

#: A pull-request workflow whose paid lane is written in the mapping form,
#: beside hosted jobs written as a list and as a mapping.
WORKFLOW = """\
"on": pull_request
jobs:
  build:
    runs-on:
      labels: [ubicloud-standard-8]
  listed:
    runs-on: [ubuntu-latest]
  grouped:
    runs-on:
      labels: ubicloud-standard-8
"""


@pytest.fixture
def estate(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    """Point the inventory at a one-workflow tree."""
    (tmp_path / "ci.yml").write_text(WORKFLOW, encoding="utf-8")
    # `raising` stays at its default, so a renamed constant fails loudly.
    monkeypatch.setattr(inv, "WORKFLOWS_DIR", tmp_path)
    return tmp_path


@pytest.mark.usefixtures("estate")
def test_a_mapping_form_paid_lane_is_discovered() -> None:
    """Find the paid lane a fork can reach, whatever form its runner takes."""
    lanes = {(name, job) for name, job, _ in placement._paid_lanes()}
    assert ("ci.yml", "build") in lanes, (
        f"the mapping-form paid lane must be discovered, got {sorted(lanes)}"
    )


@pytest.mark.usefixtures("estate")
def test_a_mapping_form_paid_lane_fails_the_fork_fallback() -> None:
    """Fail the fork-fallback rule on a paid lane that is not the expression."""
    runner = next(text for name, job, text in placement._paid_lanes() if job == "build")
    assert runner, "a mapping-form lane must not read as an empty runner"
    with pytest.raises(AssertionError, match="event-keyed expression"):
        placement.test_a_paid_lane_a_fork_can_reach_falls_back_to_a_hosted_runner(
            "ci.yml", "build", runner
        )


@pytest.mark.usefixtures("estate")
def test_the_hosted_check_reads_list_and_mapping_forms() -> None:
    """Accept a hosted list and refuse a paid mapping in the hosted-job rule."""
    tools.test_non_build_jobs_stay_github_hosted("ci.yml", "listed")
    with pytest.raises(AssertionError, match="GitHub-hosted"):
        tools.test_non_build_jobs_stay_github_hosted("ci.yml", "grouped")
