"""Contract tests for the Nixie and Merman validation toolchain."""

from __future__ import annotations

from pathlib import Path
from typing import cast

import yaml

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
WORKFLOW_PATH = REPOSITORY_ROOT / ".github" / "workflows" / "ci.yml"
MAKEFILE_PATH = REPOSITORY_ROOT / "Makefile"


def _build_steps() -> list[dict[str, object]]:
    """Return the steps from the CI build job."""
    workflow = yaml.safe_load(WORKFLOW_PATH.read_text(encoding="utf-8"))
    jobs = workflow.get("jobs")
    assert isinstance(jobs, dict), "the CI workflow must declare jobs"
    build = jobs.get("build")
    assert isinstance(build, dict), "the CI workflow must declare the build job"
    steps = build.get("steps")
    assert isinstance(steps, list), "the CI build job must declare steps"
    assert all(isinstance(step, dict) for step in steps), (
        "every CI build step must be a mapping"
    )
    return cast("list[dict[str, object]]", steps)


def _find_step(steps: list[dict[str, object]], name: str) -> dict[str, object]:
    """Return the uniquely named workflow step."""
    matches = [step for step in steps if step.get("name") == name]
    assert len(matches) == 1, f"expected one {name!r} step, found {len(matches)}"
    return matches[0]


def _nixie_recipe() -> list[str]:
    """Return the non-empty commands in the Makefile's Nixie recipe."""
    lines = MAKEFILE_PATH.read_text(encoding="utf-8").splitlines()
    target_index = lines.index("nixie:")
    recipe: list[str] = []
    for line in lines[target_index + 1 :]:
        if not line.startswith("\t"):
            break
        command = line.removeprefix("\t").strip()
        if command:
            recipe.append(command)
    return recipe


def test_ci_installs_pinned_renderers_before_running_nixie() -> None:
    """CI installs the reviewed tool versions before Mermaid validation."""
    steps = _build_steps()
    merman = _find_step(steps, "Install Merman CLI")
    nixie = _find_step(steps, "Install Nixie")
    validation = _find_step(steps, "Nixie")

    assert merman.get("run") == (
        "cargo binstall --no-confirm --locked merman-cli@0.7.0"
    ), "CI must install the locked Merman CLI 0.7.0 release"
    assert nixie.get("run") == 'uv tool install --python 3.14 "nixie-cli==1.1.0"', (
        "CI must install Nixie CLI 1.1.0"
    )
    assert validation.get("run") == "make nixie", (
        "CI must run Mermaid validation through the Makefile contract"
    )
    assert steps.index(merman) < steps.index(validation), (
        "CI must install Merman before running Nixie"
    )
    assert steps.index(nixie) < steps.index(validation), (
        "CI must install Nixie before running Nixie"
    )


def test_makefile_nixie_requires_both_installed_commands() -> None:
    """The Nixie target fails early unless both renderer commands exist."""
    assert _nixie_recipe() == [
        "$(call ensure_tool,nixie)",
        "$(call ensure_tool,merman-cli)",
        "nixie",
    ], "the Nixie recipe must require both tools before validation"
