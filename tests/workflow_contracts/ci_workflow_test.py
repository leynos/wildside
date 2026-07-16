"""Contract tests for pull-request coverage enforcement in CI."""
from __future__ import annotations

from pathlib import Path
import re

import typing as typ
import yaml

WORKFLOW_PATH = Path(__file__).resolve().parents[2] / ".github" / "workflows" / "ci.yml"
SHA_RE = re.compile(r"^[0-9a-f]{40}$")


def _assert_pinned_to_full_sha(uses: object, expected_path: str) -> None:
    """Assert ``uses`` references ``expected_path`` pinned to a full commit SHA.

    Dependabot owns shared-action SHA bumps, so the contract asserts the pin's
    shape rather than its current value. Hard-coding the SHA would fail the
    suite on every routine bump.
    """
    assert isinstance(uses, str), f"expected a 'uses' string, got {uses!r}"
    path, separator, ref = uses.partition("@")
    assert separator, f"expected {expected_path} to carry an '@' ref, got {uses!r}"
    assert path == expected_path, f"expected {expected_path}, got {path!r}"
    assert SHA_RE.fullmatch(ref), f"expected a 40-hex commit SHA, got {ref!r}"


def _load_steps(job_name: str = "coverage") -> list[dict[str, object]]:
    """Parse and return the steps for one CI job."""
    workflow = yaml.safe_load(WORKFLOW_PATH.read_text(encoding="utf-8"))
    jobs = workflow.get("jobs")
    assert isinstance(jobs, dict), "the CI workflow must declare jobs"
    job = jobs.get(job_name)
    assert isinstance(job, dict), f"the CI workflow must declare {job_name}"
    steps = job.get("steps")
    assert isinstance(steps, list), f"the {job_name} job must declare steps"
    assert all(isinstance(step, dict) for step in steps), (
        "every coverage step must be a mapping"
    )
    return typ.cast("list[dict[str, object]]", steps)


def test_build_checkout_fetches_origin_main_history() -> None:
    """The build checkout fetches branches required by Nixie discovery."""
    checkouts = [
        step
        for step in _load_steps("build")
        if str(step.get("uses", "")).startswith("actions/checkout@")
    ]
    assert len(checkouts) == 1, "the build job must have one checkout step"
    checkout = checkouts[0]
    assert checkout.get("uses") == (
        "actions/checkout@9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0"
    ), "the build checkout must preserve the reviewed action pin"
    checkout_options = checkout.get("with")
    assert isinstance(checkout_options, dict), "the build checkout needs options"
    assert checkout_options.get("persist-credentials") is False
    assert checkout_options.get("fetch-depth") == 0


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
    _assert_pinned_to_full_sha(
        check.get("uses"),
        "leynos/shared-actions/.github/actions/upload-codescene-coverage",
    )
    assert check.get("with") == {
        "format": "lcov",
        "mode": "check",
        "project-url": "https://api.codescene.io/v2/projects/70675",
        "access-token": "${{ env.CS_ACCESS_TOKEN }}",
        "installer-checksum": "${{ vars.CODESCENE_CLI_SHA256 }}",
    }, "the CodeScene check must pass the canonical project and check-mode inputs"


def test_compile_fail_binaries_bypass_nextest() -> None:
    """Compile-fail suites run directly, outside Nextest's test timeout."""
    steps = _load_steps("build")
    rust_tests = _find_step(steps, "Rust tests")
    compile_fail_tests = _find_step(steps, "Compile-fail tests")

    assert rust_tests.get("run") == (
        "# Clean stale pg-embed directories that may conflict with new runs.\n"
        "find target/ -maxdepth 1 -type d -name 'pg-embed-*' -exec rm -rf {} + "
        "2>/dev/null || true\n"
        'RUSTFLAGS="-D warnings" cargo nextest run --locked \\\n'
        "  --manifest-path backend/Cargo.toml \\\n"
        "  --all-targets --all-features \\\n"
        "  -E 'not (binary(declare_test_support_compile_fail) | "
        "binary(compile_fail_tests))'\n"
    ), "Nextest must exclude every compile-fail test binary"
    assert compile_fail_tests.get("run") == (
        "cargo test --locked --all-features -p backend --test "
        "declare_test_support_compile_fail\n"
        "cargo test --locked --features trybuild-tests -p pagination "
        "cursor_trait_bound_compile_fail_tests\n"
    ), "compile-fail suites must remain direct Cargo tests"
