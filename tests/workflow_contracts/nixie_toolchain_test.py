"""Contract tests for the Nixie and Merman validation toolchain."""

from __future__ import annotations

import typing as typ
from pathlib import Path

import pytest
import yaml
from ci_workflow_test import _assert_pinned_to_full_sha

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
WORKFLOW_PATH = REPOSITORY_ROOT / ".github" / "workflows" / "ci.yml"
MAKEFILE_PATH = REPOSITORY_ROOT / "Makefile"


def _build_steps() -> list[dict[str, object]]:
    """Return the steps from the CI build job."""
    workflow = typ.cast(
        "object", yaml.safe_load(WORKFLOW_PATH.read_text(encoding="utf-8"))
    )
    match workflow:
        case {"jobs": dict() as jobs}:
            pass
        case _:
            pytest.fail("the CI workflow must declare jobs")
    match jobs:
        case {"build": dict() as build}:
            pass
        case _:
            pytest.fail("the CI workflow must declare the build job")
    match build:
        case {"steps": list() as steps}:
            pass
        case _:
            pytest.fail("the CI build job must declare steps")
    assert all(isinstance(step, dict) for step in steps), (
        "every CI build step must be a mapping"
    )
    return typ.cast("list[dict[str, object]]", steps)


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


INSTALL_NIXIE_PATH = "leynos/shared-actions/.github/actions/install-nixie"


def test_ci_installs_pinned_renderers_before_running_nixie() -> None:
    """CI installs both renderers from the shared action before validation.

    The action downloads a checksum-verified Merman release and the pinned
    Nixie distribution, which replaced the former ``cargo binstall`` and bare
    ``uv tool install`` steps.
    """
    steps = _build_steps()
    installation = _find_step(steps, "Install Nixie and Merman")
    validation = _find_step(steps, "Nixie")

    _assert_pinned_to_full_sha(installation.get("uses"), INSTALL_NIXIE_PATH)
    assert installation.get("with") == {
        "nixie-version": "${{ env.NIXIE_VERSION }}",
        "merman-version": "${{ env.MERMAN_VERSION }}",
    }, "CI must pin both renderer versions through the shared action"
    assert validation.get("run") == "make nixie", (
        "CI must run Mermaid validation through the Makefile contract"
    )
    assert steps.index(installation) < steps.index(validation), (
        "CI must install both renderers before running Nixie"
    )


def test_ci_pins_the_renderer_versions_the_estate_reviewed() -> None:
    """The workflow environment carries the reviewed renderer pins."""
    workflow = typ.cast(
        "object", yaml.safe_load(WORKFLOW_PATH.read_text(encoding="utf-8"))
    )
    match workflow:
        case {"env": dict() as environment}:
            pass
        case _:
            pytest.fail("the CI workflow must declare workflow-level env")
    assert environment.get("NIXIE_VERSION") == "1.1.0"
    assert environment.get("MERMAN_VERSION") == "0.7.0"


def test_makefile_nixie_requires_both_installed_commands() -> None:
    """The Nixie target fails early unless both renderer commands exist."""
    assert _nixie_recipe() == [
        "$(call ensure_tool,nixie)",
        "$(call ensure_tool,merman-cli)",
        "nixie --renderer merman",
    ], "the Nixie recipe must require both tools before validation"
