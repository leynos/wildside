"""Contract tests for pull-request quality enforcement in CI."""

from __future__ import annotations

import re
from pathlib import Path

import pytest
import typed_documents as docs

SHA_RE = re.compile(r"^[0-9a-f]{40}$")

WORKFLOW_LABEL = "ci.yml"
WORKFLOW_PATH = (
    Path(__file__).resolve().parents[2] / ".github" / "workflows" / WORKFLOW_LABEL
)


def _assert_pinned_to_full_sha(uses: object, expected_path: str) -> None:
    """Assert ``uses`` references ``expected_path`` pinned to a full commit SHA.

    Dependabot owns shared-action SHA bumps, so the contract asserts the pin's
    shape rather than its current value. Hard-coding the SHA would fail the
    suite on every routine bump.

    Three other contract modules import this, so it stays here even though
    this module's own last caller went with the CodeScene changed-line gate.
    """
    assert isinstance(uses, str), f"expected a 'uses' string, got {uses!r}"
    path, separator, ref = uses.partition("@")
    assert separator, f"expected {expected_path} to carry an '@' ref, got {uses!r}"
    assert path == expected_path, f"expected {expected_path}, got {path!r}"
    assert SHA_RE.fullmatch(ref), f"expected a 40-hex commit SHA, got {ref!r}"


def _assert_gate_runs_unconditionally(job_name: str, command: str) -> None:
    """Assert a pull request cannot reach merge without ``command`` running.

    Four things have to hold together, because each defeats the others on
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
    4. Nothing wraps or relocates the script: no `shell` and no
       `working-directory`, on the step itself or in a `defaults.run`
       mapping at job or workflow level. A `shell` value is a template the
       command is substituted into, so `bash -c "{0}"; true` runs the gate
       and returns success whatever it found. A `working-directory` value
       selects which Makefile the command reaches, so the contract would
       stop being about the repository-root gate.

    The third and fourth are asserted on the keys' presence rather than on
    their values on purpose. A condition need not be spelled `false` to skip
    the gate: an ordinary looking `github.event_name == 'push'` skips it on
    exactly the event this contract exists to cover. Enumerating falsy
    spellings also invites a subtler error, since YAML parses `false` to a
    boolean whose string form is `False`, so a test comparing against
    `"false"` passes its own mutation. The same holds for a shell template:
    the safe spellings cannot be enumerated, so the reviewed workflow simply
    declares none.
    """
    workflow = _load_workflow()
    assert "pull_request" in workflow["triggers"], (
        "the workflow must trigger on pull_request, or this gate never runs "
        "on the event it exists to gate"
    )

    _assert_no_run_defaults(workflow["defaults"], WORKFLOW_LABEL)

    job = docs.workflow_job(workflow, job_name, WORKFLOW_LABEL)
    _assert_no_run_defaults(job.get("defaults"), f"the {job_name} job")
    assert "if" not in job, (
        f"the {job_name} job must carry no condition; a skipped job runs no "
        "steps and leaves every step-level assertion vacuous"
    )
    assert "continue-on-error" not in job, (
        f"the {job_name} job must not continue on error; a job that swallows "
        "its own failure reports success whatever its steps found"
    )

    invocations = [
        step
        for step in docs.job_steps(job, f"{WORKFLOW_LABEL} job {job_name!r}")
        if _whole_run_value(step) == command
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
    _assert_no_run_wrapper(invocations[0], f"the {command!r} step")


#: The two `run` keys that change what a command means rather than what it
#: says, each with what it lets past the gate. `shell` is a template the
#: command is substituted into, so `bash -c "{0}"; true` runs the gate and
#: reports success whatever it found. `working-directory` chooses which
#: Makefile the command reaches.
_RUN_WRAPPER_KEYS = {
    "shell": (
        "a shell template runs the gate and can return success whatever its verdict"
    ),
    "working-directory": (
        "a working directory can point the command at a different Makefile"
    ),
}


def _assert_no_run_wrapper(owner: dict[str, docs.JsonValue], description: str) -> None:
    """Assert ``owner`` neither wraps a gate's script nor relocates it."""
    for key, consequence in _RUN_WRAPPER_KEYS.items():
        assert key not in owner, (
            f"{description} must carry no {key!r} key at all; {consequence}"
        )


def _assert_no_run_defaults(defaults: docs.JsonValue, description: str) -> None:
    """Assert a `defaults` mapping imposes no shell or working directory.

    A missing `defaults`, and a `defaults` that declares no `run`, both leave
    nothing to assert. Anything else is read as a mapping, so a `defaults`
    that is not one fails at the boundary rather than being skipped.
    """
    if defaults is None:
        return
    label = f"{description} 'defaults'"
    run = docs.as_mapping(defaults, label).get("run")
    if run is None:
        return
    _assert_no_run_wrapper(docs.as_mapping(run, f"{label}.run"), f"{label}.run")


def _load_workflow() -> docs.Workflow:
    """Return the CI workflow with its triggers and jobs shape-checked.

    The boundary parser owns the YAML quirks, notably that an unquoted `on:`
    key parses to the boolean `True` under YAML 1.1, so no contract here has
    to know which spelling the file currently uses.
    """
    return docs.load_workflow(WORKFLOW_PATH, WORKFLOW_LABEL)


def _load_steps(job_name: str = "coverage") -> list[dict[str, docs.JsonValue]]:
    """Parse and return the steps for one CI job."""
    job = docs.workflow_job(_load_workflow(), job_name, WORKFLOW_LABEL)
    return docs.job_steps(job, f"{WORKFLOW_LABEL} job {job_name!r}")


def _whole_run_value(step: dict[str, docs.JsonValue]) -> str | None:
    """Return a step's entire ``run`` script, stripped, when it has one.

    A step without a `run` key, or with a `run` that is not a string, returns
    `None` rather than a string that could compare equal to a gate command.
    """
    run = step.get("run")
    return run.strip() if isinstance(run, str) else None


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


def _find_step(
    steps: list[dict[str, docs.JsonValue]], name: str
) -> dict[str, docs.JsonValue]:
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

    See `_assert_gate_runs_unconditionally` for the three things that have to
    hold together, and for why a condition is rejected by the presence of the
    key rather than by its value.
    """
    _assert_gate_runs_unconditionally("build", "make docs-check")


def test_coverage_generation_forces_rust_only_ratcheted_lcov() -> None:
    """The pull-request lane produces the report the ratchet compares.

    This used to assert the ordering of the CodeScene changed-line gate
    against the generation step that fed it. That gate is gone from the
    pull-request lane under CV-005, and `codescene_coverage_baseline_test.py`
    now holds its absence. What survives is the half that was never about
    CodeScene: the inputs deciding which number is produced, pinned
    verbatim, because the publisher writes the ratchet baseline from the
    same set and a lane that drifts from it compares two different
    measurements.
    """
    generation = _find_step(_load_steps(), "Generate Rust coverage")
    assert generation.get("with") == {
        "language": "rust",
        "output-path": "lcov.info",
        "format": "lcov",
        "use-cargo-nextest": "true",
        "features": "example-data metrics test-support",
        "with-ratchet": "true",
        "cache-provider": "external",
    }, "coverage generation must force Rust-only ratcheted LCOV"


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
