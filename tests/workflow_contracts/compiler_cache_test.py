"""Contract tests for the compiler cache across the Rust jobs.

`setup-rust` owns sccache here: it installs it, selects the backend by runner,
starts the server with a 60 s startup timeout, names it as the rustc wrapper,
and, if the server will not start, falls back to an uncached build instead of
failing the job (shared-actions #546). These contracts hold that arrangement:
one reviewed `setup-rust` call per job with the inputs and id the report reads,
none of the hand-rolled pieces it replaced surviving beside it, and a
statistics step that reads what the action reports and stands down on a
fallback.
"""

from __future__ import annotations

import pytest
import workflow_inventory as inv

#: The shared-actions commit `setup-rust` must pin: #546, which sits on #523.
#: Asserted by value, so a bump has to update it and someone has to confirm the
#: new revision still leaves this repository the sole owner of its caches.
SETUP_RUST_PIN = "6cec89bac47a21cf756d68d638a9a510998e57f8"

SETUP_STEP = "Install Rust toolchain"
SETUP_ID = "setup-rust"
REPORT_STEP = "Record compiler-cache effectiveness"
NOT_FALLBACK = "steps.setup-rust.outputs.sccache-status != 'fallback'"

#: Each Rust job and the `expect-cache` it must declare. `ci.yml` jobs have a
#: fork arm on a GitHub-hosted runner, so they take whichever backend the runner
#: offers; `coverage-upload` runs only on the managed runner and must get its
#: proxy.
RUST_JOBS = (
    ("ci.yml", "build", "any"),
    ("ci.yml", "coverage", "any"),
    ("coverage-main.yml", "coverage-upload", "ubicloud"),
)

#: Job-level variables only a job that wires sccache by hand sets. `setup-rust`
#: exports the wrapper and selects the backend itself, so either surviving means
#: the job still carries a second owner.
RETIRED_JOB_ENV = ("RUSTC_WRAPPER", "SCCACHE_GHA_ENABLED", "SCCACHE_CONF")


def _steps(filename: str, job_id: str) -> list[dict[str, object]]:
    """Return the parsed steps of one job in one workflow file."""
    return inv.job_steps(inv.load_workflow(filename)["jobs"][job_id])


@pytest.mark.parametrize(("filename", "job_id", "expect"), RUST_JOBS)
def test_setup_rust_owns_the_compiler_cache(
    filename: str, job_id: str, expect: str
) -> None:
    """One reviewed call, with sccache left on and the id the report reads."""
    steps = _steps(filename, job_id)
    setup = inv.find_step(steps, SETUP_STEP)
    uses = str(setup.get("uses", ""))
    assert uses.endswith(f"/setup-rust@{SETUP_RUST_PIN}"), (
        f"{filename}:{job_id} must pin setup-rust at {SETUP_RUST_PIN}; found {uses}"
    )
    assert setup.get("id") == SETUP_ID, (
        f"{filename}:{job_id} setup-rust must carry the id {SETUP_ID!r}"
    )
    options = setup.get("with")
    assert isinstance(options, dict), f"{filename}:{job_id} needs setup-rust inputs"
    assert options.get("expect-cache") == expect, (
        f"{filename}:{job_id} must set expect-cache: {expect}"
    )
    assert options.get("cache-provider") == "external", (
        f"{filename}:{job_id} owns its caches, so setup-rust must not"
    )
    assert str(options.get("use-sccache", "true")).lower() == "true", (
        f"{filename}:{job_id} must leave sccache to setup-rust, not turn it off"
    )


@pytest.mark.parametrize(("filename", "job_id", "expect"), RUST_JOBS)
def test_no_hand_rolled_compiler_cache_survives_beside_setup_rust(
    filename: str, job_id: str, expect: str
) -> None:
    """Two owners would start two servers, and the older one would win."""
    del expect
    job = inv.load_workflow(filename)["jobs"][job_id]
    environment = job.get("env", {}) or {}
    for variable in RETIRED_JOB_ENV:
        assert variable not in environment, (
            f"{filename}:{job_id} must not set {variable} at job level; "
            "setup-rust owns it"
        )
    for step in inv.job_steps(job):
        script = str(step.get("run", ""))
        uses = str(step.get("uses", ""))
        label = f"{filename}:{job_id} step {step.get('name')!r}"
        assert "start-compiler-cache" not in script, f"{label} starts the server"
        assert "sccache --zero-stats" not in script, f"{label} zeroes the counters"
        assert "sccache --start-server" not in script, f"{label} starts the server"
        exports_proxy = uses.startswith("actions/github-script") and (
            "ACTIONS_CACHE_URL" in str((step.get("with") or {}).get("script", ""))
        )
        assert not exports_proxy, f"{label} exports the cache proxy by hand"


@pytest.mark.parametrize(("filename", "job_id", "expect"), RUST_JOBS)
def test_the_toolchain_is_set_up_before_anything_reports_statistics(
    filename: str, job_id: str, expect: str
) -> None:
    """Statistics describe the job's own compilation, so setup-rust comes first."""
    del expect
    steps = _steps(filename, job_id)
    assert inv.step_index(steps, SETUP_STEP) < inv.step_index(steps, REPORT_STEP), (
        f"{filename}:{job_id} must set up Rust before reporting statistics"
    )


@pytest.mark.parametrize(("filename", "job_id", "expect"), RUST_JOBS)
def test_the_statistics_step_reads_the_action_and_stands_down_on_a_fallback(
    filename: str, job_id: str, expect: str
) -> None:
    """A server that fell back has no statistics, and an empty report misleads.

    The step runs under `always()` and the fallback guard, names the backend
    `setup-rust` chose (`Cache location` reads `ghac` for the proxy and for
    GitHub's own service alike), and still reports zero compile requests as a
    failed integration rather than a cold cache.
    """
    del expect
    report = inv.find_step(_steps(filename, job_id), REPORT_STEP)
    assert report.get("if") == f"always() && {NOT_FALLBACK}", (
        f"{filename}:{job_id} must report on failure too, and stand down on a "
        "setup-rust fallback"
    )
    env = report.get("env")
    assert isinstance(env, dict), f"{filename}:{job_id} report needs an env block"
    assert env.get("SCCACHE_BACKEND") == (
        "${{ steps.setup-rust.outputs.cache-backend }}"
    ), f"{filename}:{job_id} must hand setup-rust's cache-backend to the report"
    script = str(report.get("run", ""))
    assert "printf 'backend: %s\\n' \"${SCCACHE_BACKEND}\"" in script, (
        f"{filename}:{job_id} must print the backend it reports"
    )
    assert "sccache --show-stats" in script, (
        f"{filename}:{job_id} must report sccache's statistics"
    )
