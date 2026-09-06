"""Contract tests for pull-request quality enforcement in CI."""
from __future__ import annotations

from pathlib import Path
import re
import typing as typ

import pytest
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


def _assert_gate_runs_unconditionally(job_name: str, command: str) -> None:
    """Assert a pull request cannot reach merge without ``command`` running.

    Three things have to hold together, because each defeats the others on
    its own:

    1. The workflow triggers on `pull_request`. Without it nothing here runs
       and every other assertion is about a workflow no pull request invokes.
    2. Exactly one step in ``job_name`` has ``command`` as its **whole** `run`
       value. Matching a line within a multiline script is satisfied by
       `if false; then <command>; fi`, and matching a substring is satisfied
       by `<command> || true`.
    3. Neither the job nor that step carries an `if` or `continue-on-error`
       key **at all**. A condition skips the gate; `continue-on-error` runs
       it and discards the verdict. Both leave a green pull request that the
       gate never actually held.

    The third is asserted on the keys' presence rather than on their values
    on purpose. A condition need not be spelled `false` to skip the gate: an
    ordinary looking `github.event_name == 'push'` skips it on exactly the
    event this contract exists to cover. Enumerating falsy spellings also
    invites a subtler error, since YAML parses `false` to a boolean whose
    string form is `False`, so a test comparing against `"false"` passes its
    own mutation.
    """
    workflow = yaml.safe_load(WORKFLOW_PATH.read_text(encoding="utf-8"))
    # An unquoted `on:` key parses as the boolean True under YAML 1.1, so
    # both spellings are accepted here rather than depending on the quoting.
    triggers = workflow.get("on", workflow.get(True))
    assert isinstance(triggers, dict), "the CI workflow must declare triggers"
    assert "pull_request" in triggers, (
        "the workflow must trigger on pull_request, or this gate never runs "
        "on the event it exists to gate"
    )

    jobs = workflow.get("jobs")
    assert isinstance(jobs, dict), "the CI workflow must declare jobs"
    job = jobs.get(job_name)
    assert isinstance(job, dict), f"the CI workflow must declare {job_name}"
    assert "if" not in job, (
        f"the {job_name} job must carry no condition; a skipped job runs no "
        "steps and leaves every step-level assertion vacuous"
    )
    assert "continue-on-error" not in job, (
        f"the {job_name} job must not continue on error; a job that swallows "
        "its own failure reports success whatever its steps found"
    )

    steps = typ.cast("list[dict[str, object]]", job.get("steps"))
    invocations = [
        step
        for step in steps
        if isinstance(step.get("run"), str)
        and typ.cast("str", step["run"]).strip() == command
    ]
    assert len(invocations) == 1, (
        f"expected exactly one step in {job_name} whose whole run value is "
        f"{command!r}, found {len(invocations)}"
    )
    assert "if" not in invocations[0], (
        f"the {command!r} step must carry no condition at all"
    )
    assert "continue-on-error" not in invocations[0], (
        f"the {command!r} step must not continue on error; running the gate "
        "and discarding its verdict is the same as not running it"
    )


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

def test_build_runs_the_typedoc_documentation_gate() -> None:
    """Pull requests must reject undocumented JavaScript and TypeScript APIs.

    The assertion looks for the gate's command rather than its step name. A
    step name is prose: renaming it, or deleting the step while leaving a
    similarly named neighbour, must not be able to satisfy this contract.
    Only a step that actually invokes ``make docs-check`` does.
    """
    invocations = [
        step
        for step in _load_steps("build")
        if isinstance(step.get("run"), str)
        and any(
            line.strip() == "make docs-check"
            for line in typ.cast("str", step["run"]).splitlines()
        )
    ]
    assert len(invocations) == 1, (
        "expected exactly one build step running 'make docs-check', "
        f"found {len(invocations)}"
    )
    assert "if" not in invocations[0], (
        "the documentation gate must run unconditionally, so it cannot carry "
        "an 'if' guard"
    )
    assert invocations[0].get("continue-on-error") in (None, False), (
        "the documentation gate must fail the job rather than continue on error"
    )
def test_codescene_check_immediately_follows_coverage_generation() -> None:
    """The changed-line gate consumes the LCOV report produced just before it."""
    steps = _load_steps()
    generation = _find_step(steps, "Generate Rust coverage")
    check = _find_step(steps, "Check coverage against CodeScene gates")
    assert steps.index(check) == steps.index(generation) + 1, (
        "the CodeScene check must immediately follow coverage generation"
    )
    assert generation.get("with") == {
        "language": "rust",
        "output-path": "lcov.info",
        "format": "lcov",
        "use-cargo-nextest": "true",
        "features": "example-data metrics test-support",
        "with-ratchet": "true",
        "cache-provider": "external",
    }, "coverage generation must force Rust-only LCOV via language: rust"


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


# Each Python gate runs through its Makefile target so local runs and CI
# exercise one definition; asserting the exact command keeps them in lockstep.
PYTHON_GATE_STEPS = (
    ("Python format check", "make check-fmt-python"),
    ("Python lint", "make lint-python"),
    ("Type check", "make typecheck"),
)


def test_build_runs_the_workflow_lint_script_tests() -> None:
    """The lint-actions script's own suite must run in CI.

    `make test` gathers it, but the workflow invokes the individual test
    targets rather than the aggregate, so a suite that is not named here never
    runs. That is worth a contract because the failure is silent: the tests
    pass locally and simply never execute on a pull request.

    The whole `run` value must be the command, not merely contain it. A step
    whose script wraps the command in `if false; then ... fi` satisfies a
    substring or per-line search while running nothing, and the `if` key check
    below cannot see a condition written in shell.
    """
    _assert_gate_runs_unconditionally("build", "make test-lint-actions")


@pytest.mark.parametrize(("step_name", "command"), PYTHON_GATE_STEPS)
def test_build_runs_python_quality_gates(step_name: str, command: str) -> None:
    """CI drives every Python quality gate through its Makefile target."""
    step = _find_step(_load_steps("build"), step_name)

    assert step.get("run") == command, (
        f"the {step_name!r} step must run {command!r} so CI and local runs"
        " share one gate definition"
    )
