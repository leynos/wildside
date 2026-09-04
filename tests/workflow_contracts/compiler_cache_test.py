"""Contract tests for the compiler cache across the Rust jobs.

These encode where the `sccache` server may start, which backend it must bind,
what evidence a run has to leave behind, and which cache key owns the archive
that stores the binary. They are separate from the cache-ownership contracts
because the failure they guard against is different: not two writers competing
for one key, but a cache that reports success while its objects go elsewhere.
"""

from __future__ import annotations

import pathlib
import typing as typ

import pytest
import workflow_inventory as inv

#: Every Rust job starts its compiler cache through this one script, so the
#: digest pins and the backend guard have a single place to change.
COMPILER_CACHE_SCRIPT = (
    pathlib.Path(__file__).resolve().parents[2] / "scripts" / "start-compiler-cache.sh"
)


def _compiler_cache_script() -> str:
    """Return the compiler-cache start script's source."""
    return COMPILER_CACHE_SCRIPT.read_text(encoding="utf-8")


def _structured_composer(steps: list[dict[str, typ.Any]], key: str) -> str | None:
    """Return `key`'s value where a step declares it as a structured `env` entry."""
    values = (step.get("env", {}) for step in steps)
    matches = (env[key] for env in values if isinstance(env, dict) and key in env)
    return next((str(value) for value in matches), None)


def _script_composer(steps: list[dict[str, typ.Any]], key: str) -> str | None:
    """Return the `Set cache keys` line that assigns `key`.

    The `printf` that builds it continues onto the next line, so the scalar is
    unwrapped before it is split; matching a line at a time would find the
    format string without its arguments.
    """
    script = str(inv.find_step(steps, "Set cache keys")["run"])
    lines = script.replace("\\\n", " ").splitlines()
    return next((line for line in lines if f"{key}=" in line), None)


def _cache_key_composer(steps: list[dict[str, typ.Any]], key: str) -> str:
    """Return the text that builds `key`, and nothing else in the job.

    A cache key is composed one of two ways here. `TOOL_PINS` is a structured
    `env` value on the step that consumes it. `COVERAGE_TOOLS_CACHE_KEY` is a
    `printf` in the `Set cache keys` shell script.
    """
    composer = _structured_composer(steps, key) or _script_composer(steps, key)
    if composer is None:
        message = f"no step composes {key}"
        raise AssertionError(message)
    return composer


RUST_JOBS = (
    ("ci.yml", "build"),
    ("ci.yml", "coverage"),
    ("coverage-main.yml", "coverage-upload"),
)


@pytest.mark.parametrize(("filename", "job_id"), RUST_JOBS)
def test_rust_jobs_export_the_compiler_wrapper(filename: str, job_id: str) -> None:
    """The shared setup action installs sccache but never exports the wrapper.

    Without `RUSTC_WRAPPER` the compiler ignores sccache entirely and the job
    reports a healthy cache while recompiling everything, which is the worst
    of both outcomes now that no `target` tree is archived.
    """
    job = inv.load_workflow(filename)["jobs"][job_id]
    environment = job.get("env", {})
    assert environment.get("RUSTC_WRAPPER") == "sccache", (
        f"{filename}:{job_id} must export RUSTC_WRAPPER"
    )
    assert environment.get("SCCACHE_GHA_ENABLED") == "true", (
        f"{filename}:{job_id} must enable sccache's GitHub Actions backend"
    )


@pytest.mark.parametrize(("filename", "job_id"), RUST_JOBS)
def test_cache_credentials_are_exported_before_the_toolchain(
    filename: str, job_id: str
) -> None:
    """A plain `run:` step cannot see the managed runner's cache proxy.

    Only an action step can, so the credentials must be republished before the
    toolchain setup that installs sccache runs.
    """
    steps = inv.job_steps(inv.load_workflow(filename)["jobs"][job_id])
    export = inv.step_index(steps, "Export cache credentials for sccache")
    toolchain = inv.step_index(steps, "Install Rust toolchain")
    assert export < toolchain, (
        f"{filename}:{job_id} must export the cache credentials before setup-rust"
    )
    exported = str(inv.find_step(steps, "Export cache credentials for sccache"))
    for variable in ("ACTIONS_CACHE_URL", "ACTIONS_RUNTIME_TOKEN"):
        assert variable in exported, (
            f"{filename}:{job_id} must republish {variable} for sccache"
        )
    # A non-empty value selects the v2 cache service, which bypasses the
    # managed runner's proxy and sends sccache's objects somewhere the rest of
    # the estate cannot read. Assert the exact call, not just the name.
    assert "core.exportVariable('ACTIONS_CACHE_SERVICE_V2', '')" in exported, (
        f"{filename}:{job_id} must clear ACTIONS_CACHE_SERVICE_V2, not merely set it"
    )


@pytest.mark.parametrize(("filename", "job_id"), RUST_JOBS)
def test_compiler_cache_statistics_bracket_the_build(
    filename: str, job_id: str
) -> None:
    """Statistics are the only evidence that the wrapper actually engaged."""
    steps = inv.job_steps(inv.load_workflow(filename)["jobs"][job_id])
    reset = inv.step_index(steps, "Reset compiler-cache counters")
    report = inv.step_index(steps, "Record compiler-cache effectiveness")
    assert reset < report, f"{filename}:{job_id} must reset counters before reporting"
    assert inv.find_step(steps, "Record compiler-cache effectiveness").get("if") == (
        "always()"
    ), f"{filename}:{job_id} must report statistics even when the build fails"


@pytest.mark.parametrize(("filename", "job_id"), RUST_JOBS)
def test_the_compiler_cache_server_starts_from_a_run_step(
    filename: str, job_id: str
) -> None:
    """The server binds its backend at start, so where it starts decides which.

    `setup-rust` starts one through `mozilla-actions/sccache-action`, whose
    last act is to write `ACTIONS_CACHE_SERVICE_V2=on` back to the environment
    file along with GitHub's own results URL and token. That clobbers this
    job's earlier credential export, for that server and every step after it,
    and the objects go to GitHub instead of the managed store. Passing
    `use-sccache: false` keeps that step out of the job. The first Wildside run
    to wire sccache made exactly this mistake: 14,480 compile requests produced
    no objects in the managed cache.
    """
    steps = inv.job_steps(inv.load_workflow(filename)["jobs"][job_id])
    export = inv.step_index(steps, "Export cache credentials for sccache")
    start = inv.step_index(steps, "Install and start the compiler cache")
    toolchain = inv.step_index(steps, "Install Rust toolchain")
    reset = inv.step_index(steps, "Reset compiler-cache counters")

    starter = inv.find_step(steps, "Install and start the compiler cache")
    assert "run" in starter, "the server must start from a run step, not an action"
    assert COMPILER_CACHE_SCRIPT.name in str(starter["run"]), (
        f"{filename}:{job_id} must start the server through "
        f"{COMPILER_CACHE_SCRIPT.name}"
    )
    assert "--start-server" in _compiler_cache_script(), (
        f"{COMPILER_CACHE_SCRIPT.name} must start the server explicitly"
    )
    assert export < start < toolchain < reset, (
        f"{filename}:{job_id} must export credentials, start the server, set up "
        "the toolchain, and only then reset the counters"
    )

    setup = inv.find_step(steps, "Install Rust toolchain")
    options = setup.get("with")
    assert isinstance(options, dict), f"{filename}:{job_id} needs setup-rust inputs"
    assert options.get("use-sccache") == "false", (
        f"{filename}:{job_id} must stop setup-rust starting a second server"
    )


@pytest.mark.parametrize(("filename", "job_id"), RUST_JOBS)
def test_the_compiler_cache_start_verifies_its_backend(
    filename: str, job_id: str
) -> None:
    """A server on the wrong backend must fail the job, not be measured.

    Both assertions target the guard rather than a mention of it. `ghac`
    appears in the script's prose, and `sha256sum` computes a digest that a
    script could then ignore, so neither name alone proves the protection is
    still wired.
    """
    steps = inv.job_steps(inv.load_workflow(filename)["jobs"][job_id])
    starter = inv.find_step(steps, "Install and start the compiler cache")
    invocation = str(starter["run"])
    assert COMPILER_CACHE_SCRIPT.name in invocation, (
        f"{filename}:{job_id} must delegate to {COMPILER_CACHE_SCRIPT.name}"
    )
    script = _compiler_cache_script()
    assert "ghac*)" in script, (
        "the start script must branch on the backend, not merely name it"
    )
    assert '"$actual_sha" != "$expected_sha"' in script, (
        "the start script must compare the archive digest against its pin"
    )
    assert "sha256sum" in script, "the start script must compute the archive digest"
    # `sccache` is named as RUSTC_WRAPPER, so every later step resolves it
    # through PATH. The script installs it into a directory that not every
    # runner image carries, so it must publish that directory itself rather
    # than inherit it.
    assert "GITHUB_PATH" in script, (
        "the start script must publish its install directory to later steps"
    )


@pytest.mark.parametrize(
    ("filename", "job_id", "key"),
    [
        ("ci.yml", "build", "TOOL_PINS"),
        ("ci.yml", "coverage", "COVERAGE_TOOLS_CACHE_KEY"),
        ("coverage-main.yml", "coverage-upload", "COVERAGE_TOOLS_CACHE_KEY"),
    ],
)
def test_the_sccache_pin_feeds_the_archive_that_stores_it(
    filename: str, job_id: str, key: str
) -> None:
    """Every job caches `~/.local/bin`, which is where sccache is installed.

    Without the pin in the key, bumping `SCCACHE_VERSION` leaves the warm
    archive valid and the old binary is restored under the new pin. The start
    script's version probe then re-downloads on every run until an unrelated
    pin happens to move.

    The assertion reads the composer itself rather than the job as a whole.
    Every one of these jobs also names `SCCACHE_VERSION` in its `env` block,
    for the start script's benefit, so a search over the serialized job would
    stay true after the pin was removed from the key.
    """
    steps = inv.job_steps(inv.load_workflow(filename)["jobs"][job_id])
    composer = _cache_key_composer(steps, key)
    assert "SCCACHE_VERSION" in composer, (
        f"{filename}:{job_id} must feed the sccache pin into {key}, "
        f"not merely name it elsewhere in the job"
    )
