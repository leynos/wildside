"""Contract tests for cache ownership across the workflow estate.

These encode the rules the runner-platform rollout depends on: one owner per
mutable path, one writer per key, an explainable pinned cache action, no
archived compiler output, and pull requests that read the trunk generation
without publishing a competing one.
"""

from __future__ import annotations

import pathlib
import re
import typing as typ

import pytest
import workflow_inventory as inv
import yaml

#: The uv download store, tool environments, and shims must be cached
#: together. Restoring the environment store alone leaves `uv tool install`
#: reporting success while the command it installed is missing.
UV_LAYERS = ("~/.cache/uv", "~/.local/share/uv", "~/.local/bin")

#: Every Rust job starts its compiler cache through this one script, so the
#: digest pins and the backend guard have a single place to change.
COMPILER_CACHE_SCRIPT = (
    pathlib.Path(__file__).resolve().parents[2] / "scripts" / "start-compiler-cache.sh"
)


def _compiler_cache_script() -> str:
    """Return the compiler-cache start script's source."""
    return COMPILER_CACHE_SCRIPT.read_text(encoding="utf-8")


_FULL_SHA = re.compile(r"^[0-9a-f]{40}$")


def _cache_steps() -> list[tuple[str, str, dict[str, typ.Any]]]:
    """Return every cache step in the estate with its workflow and job."""
    return [
        (filename, job_id, step)
        for filename, job_id, job in inv.iter_jobs()
        for step in inv.job_steps(job)
        if inv.is_cache_step(step)
    ]


def test_no_workflow_references_the_deprecated_ubicloud_cache_fork() -> None:
    """The deprecated cache fork is gone; one pinned upstream action remains.

    The fork needs runtime variables that only the managed VM supplies, so a
    workflow carrying it cannot be moved between runner providers unchanged.
    """
    offenders = [
        filename
        for filename in inv.workflow_filenames()
        if "ubicloud/cache"
        in (inv.WORKFLOWS_DIR / filename).read_text(encoding="utf-8")
    ]
    assert offenders == [], f"these workflows still use ubicloud/cache: {offenders}"


def test_every_cache_step_uses_one_pinned_action_version() -> None:
    """Cache traffic goes through one reviewed, immutable action version.

    The value is Dependabot's to move, so this asserts that every cache step
    agrees on a full commit SHA rather than naming a particular one.
    """
    refs = {
        str(step.get("uses", "")).partition("@")[2] for _, _, step in _cache_steps()
    }
    assert all(_FULL_SHA.fullmatch(ref) for ref in refs), (
        f"cache steps must pin a full commit SHA, found: {sorted(refs)}"
    )
    assert len(refs) == 1, f"cache steps disagree on the action version: {refs}"


def test_no_job_archives_a_target_tree() -> None:
    """A cached `target` tree duplicates the compiler's own output ownership.

    It is also invalidated far more often than the registry it would travel
    with, so the archive costs transfer time on nearly every run.
    """
    offenders = [
        (filename, job_id, step.get("name"), path)
        for filename, job_id, step in _cache_steps()
        for path in inv.cache_paths(step)
        if path == "target" or path.startswith("target/")
        if path != "target/pg-worker-root"
    ]
    assert offenders == [], f"these cache steps archive compiler output: {offenders}"


def _owned_paths(job: dict[str, typ.Any]) -> list[tuple[str, str]]:
    """Return each cached path in a job alongside the key that owns it.

    A restore and its matching save are the same owner, so ownership is
    identified by the key a step reserves rather than by the step itself.
    """
    prefixes = inv.cache_key_prefixes(job)
    return [
        (path, inv.resolved_cache_key(step, prefixes))
        for step in inv.job_steps(job)
        if inv.is_cache_step(step)
        for path in inv.cache_paths(step)
    ]


def _contested_paths(job: dict[str, typ.Any]) -> list[tuple[str, str, str]]:
    """Return ``(path, first owner, rival owner)`` for every contested path."""
    seen: dict[str, str] = {}
    contested: list[tuple[str, str, str]] = []
    for path, owner in _owned_paths(job):
        if seen.get(path, owner) != owner:
            contested.append((path, seen[path], owner))
        seen[path] = owner
    return contested


def test_each_mutable_path_has_exactly_one_owner_per_job() -> None:
    """Two cache steps claiming one path race to define its contents."""
    for filename, job_id, job in inv.iter_jobs():
        contested = _contested_paths(job)
        assert not contested, (
            f"{filename}:{job_id} has more than one owner for: {contested}"
        )


def test_the_uv_layers_are_cached_together() -> None:
    """Any job caching the uv download store caches its environments and shims."""
    for filename, job_id, job in inv.iter_jobs():
        for step in inv.job_steps(job):
            paths = inv.cache_paths(step)
            if "~/.cache/uv" not in paths:
                continue
            missing = [layer for layer in UV_LAYERS if layer not in paths]
            assert missing == [], (
                f"{filename}:{job_id} step {step.get('name')!r} caches the uv "
                f"download store without {missing}"
            )


def test_cache_saves_are_restricted_to_trunk() -> None:
    """Pull requests restore the trusted generation and never publish one."""
    for filename, job_id, job in inv.iter_jobs():
        for step in inv.job_steps(job):
            if inv.step_action(step) != "actions/cache/save":
                continue
            condition = step.get("if")
            assert isinstance(condition, str), (
                f"{filename}:{job_id} step {step.get('name')!r} must guard its save"
            )
            assert "refs/heads/main" in condition, (
                f"{filename}:{job_id} step {step.get('name')!r} must only save on main"
            )


def test_each_cache_key_has_exactly_one_writer() -> None:
    """No two jobs race to reserve the same key.

    Concurrent writers produce the "Unable to reserve cache" signature and
    leave whichever job lost the race paying for an upload it discards.
    """
    writers: dict[str, list[str]] = {}
    for filename, job_id, job in inv.iter_jobs():
        prefixes = inv.cache_key_prefixes(job)
        for step in inv.job_steps(job):
            if inv.step_action(step) != "actions/cache/save":
                continue
            key = inv.resolved_cache_key(step, prefixes)
            writers.setdefault(key, []).append(f"{filename}:{job_id}")
    contested = {key: jobs for key, jobs in writers.items() if len(jobs) > 1}
    assert contested == {}, f"these keys have competing writers: {contested}"


def test_every_restore_precedes_the_first_installer() -> None:
    """Cache wiring comes before the work it is meant to make unnecessary."""
    for filename, job_id, job in inv.iter_jobs():
        steps = inv.job_steps(job)
        restores = [
            index
            for index, step in enumerate(steps)
            if inv.step_action(step) == "actions/cache/restore"
        ]
        installers = [
            index
            for index, step in enumerate(steps)
            if str(step.get("name", "")).startswith("Install")
        ]
        if not restores or not installers:
            continue
        assert max(restores) < min(installers), (
            f"{filename}:{job_id} installs before its caches are restored"
        )


@pytest.mark.parametrize(
    ("filename", "job_id", "expected"),
    [
        ("ci.yml", "build", "TOOLS_CACHE_KEY"),
        ("ci.yml", "coverage", "COVERAGE_TOOLS_CACHE_KEY"),
        ("coverage-main.yml", "coverage-upload", "COVERAGE_TOOLS_CACHE_KEY"),
    ],
)
def test_tool_cache_keys_carry_the_runner_image_identity(
    filename: str, job_id: str, expected: str
) -> None:
    """A prebuilt binary must not be restored onto an image that cannot run it.

    The image line decides which glibc is available, so it belongs in the key
    of every archive that holds an executable.
    """
    job = inv.load_workflow(filename)["jobs"][job_id]
    rendered = yaml.safe_dump(job)
    assert expected in rendered, f"{filename}:{job_id} must compose {expected}"
    assert "ImageOS" in rendered, (
        f"{filename}:{job_id} must include the runner image line in {expected}"
    )
    assert "runner.environment" in rendered, (
        f"{filename}:{job_id} must separate managed and GitHub-hosted archives"
    )


def test_actionlint_registers_every_managed_label_in_use() -> None:
    """An unregistered label makes actionlint reject the workflow that uses it."""
    config = yaml.safe_load(inv.ACTIONLINT_CONFIG.read_text(encoding="utf-8"))
    registered = set(config["self-hosted-runner"]["labels"])
    used = {
        job["runs-on"]
        for _, _, job in inv.iter_jobs()
        if isinstance(job.get("runs-on"), str)
        and job["runs-on"] not in inv.GITHUB_HOSTED_LABELS
    }
    assert used <= registered, f"unregistered runner labels in use: {used - registered}"


#: The Rust jobs. Each compiles enough that an unengaged compiler cache is the
#: difference between a warm run and a cold one.
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
    """
    job = inv.load_workflow(filename)["jobs"][job_id]
    rendered = yaml.safe_dump(job)
    assert key in rendered, f"{filename}:{job_id} must compose {key}"
    assert "SCCACHE_VERSION" in rendered, (
        f"{filename}:{job_id} must feed the sccache pin into {key}"
    )
