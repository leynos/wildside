"""Contract tests for how CI obtains its executables and places its jobs.

CI must not compile a tool it could download. Every installer here is a
version-pinned, checksum-verified distribution, every job that needs a tool
installs it before its first use, and jobs that are not builds stay on
GitHub-hosted runners.
"""

from __future__ import annotations

import re
import typing as typ

import pytest
import workflow_inventory as inv

#: `cargo binstall` compiles from source unless the strategy list forbids it.
BINSTALL_FAIL_CLOSED = "--strategies crate-meta-data,quick-install"

#: Non-build jobs. Scheduled, API-bound, and administrative work sits off the
#: developer feedback path, so the queue contention that motivates the managed
#: runners does not apply to it.
GITHUB_HOSTED_JOBS = (
    ("audit.yml", "audit"),
    ("delayed-pr-comment.yml", "delay_and_comment"),
)

#: Callers of externally owned reusable workflows choose no runner; the callee
#: does. Both callees already run on GitHub-hosted runners.
REUSABLE_CALLER_JOBS = (
    ("mutation-testing.yml", "mutation"),
    ("dependabot-automerge.yml", "automerge"),
)

#: Each pair names an installing step and the first step that needs what it
#: installed.
BUILD_INSTALLER_ORDER = (
    ("Install Rust toolchain", "Install cargo-audit"),
    ("Install cargo-audit", "Audit"),
    ("Install Nixie and Merman", "Nixie"),
    ("Install yamllint", "YAML lint"),
    ("Install workflow linters", "Workflow lint"),
    ("Install Whitaker", "Whitaker lint"),
    ("Install nextest", "Rust tests"),
    # pg_worker now arrives through `cargo binstall`, which setup-rust
    # installs, so the toolchain step is a hard prerequisite rather than a
    # convention. The old `cargo install` needed only cargo itself.
    ("Install Rust toolchain", "Install pg_worker binary"),
    ("Install pg_worker binary", "Rust tests"),
    ("Restore PostgreSQL embedded binaries", "Warm PostgreSQL embedded binary cache"),
)

_SHARED_ACTION_REFERENCE = re.compile(
    r"leynos/shared-actions/(?P<path>[^@\s]+)@(?P<ref>\S+)"
)
_FULL_SHA = re.compile(r"^[0-9a-f]{40}$")


def _cache_steps_with_paths() -> list[tuple[str, str, dict[str, object]]]:
    """Return every cache step in the estate with its workflow and job."""
    return [
        (filename, job_id, step)
        for filename, job_id, job in inv.iter_jobs()
        for step in inv.job_steps(job)
        if inv.is_cache_step(step)
    ]


def _workflow_text(filename: str) -> str:
    """Return a workflow file's raw text."""
    return (inv.WORKFLOWS_DIR / filename).read_text(encoding="utf-8")


def _build_steps() -> list[dict[str, typ.Any]]:
    """Return the CI build job's steps."""
    return inv.job_steps(inv.load_workflow("ci.yml")["jobs"]["build"])


def test_no_workflow_builds_a_tool_from_source() -> None:
    """`cargo install` compiles a crate CI should be downloading."""
    offenders = [
        (filename, job_id, step_name, command)
        for filename, job_id, step_name, command in inv.iter_step_commands()
        if "cargo install" in command
    ]
    assert offenders == [], f"these steps compile a tool from source: {offenders}"


def test_every_binstall_invocation_fails_closed() -> None:
    """Without an explicit strategy list, binstall falls back to compiling."""
    offenders = [
        (filename, job_id, step_name, command)
        for filename, job_id, step_name, command in inv.iter_step_commands()
        if "cargo binstall" in command and BINSTALL_FAIL_CLOSED not in command
    ]
    assert offenders == [], (
        f"these invocations must pass {BINSTALL_FAIL_CLOSED}: {offenders}"
    )


def test_install_action_never_falls_back_to_a_source_build() -> None:
    """`taiki-e/install-action` compiles the tool unless told to fail closed."""
    for filename, job_id, job in inv.iter_jobs():
        for step in inv.job_steps(job):
            if inv.step_action(step) != "taiki-e/install-action":
                continue
            options = step.get("with")
            assert isinstance(options, dict), f"{filename}:{job_id} needs step inputs"
            assert options.get("fallback") == "none", (
                f"{filename}:{job_id} step {step.get('name')!r} must set fallback: none"
            )


def _shared_action_references() -> list[tuple[str, str, str]]:
    """Return every `(filename, action path, ref)` the estate points at."""
    return [
        (filename, match.group("path"), match.group("ref"))
        for filename in inv.workflow_filenames()
        for match in _SHARED_ACTION_REFERENCE.finditer(_workflow_text(filename))
    ]


def test_every_shared_action_reference_is_pinned_to_a_full_sha() -> None:
    """A mutable ref would let an unreviewed change reach CI silently."""
    mutable = [
        reference
        for reference in _shared_action_references()
        if not _FULL_SHA.fullmatch(reference[2])
    ]
    assert mutable == [], f"these shared-action refs are not full SHAs: {mutable}"


def test_the_estate_calls_one_shared_actions_commit() -> None:
    """One reviewed commit supplies every shared action the estate calls.

    The value itself is Dependabot's to move, so this asserts agreement
    between the references rather than a particular SHA.
    """
    refs = {reference[2] for reference in _shared_action_references()}
    assert len(refs) == 1, f"the estate spans several shared-actions commits: {refs}"


def test_whitaker_comes_from_the_shared_installer_action() -> None:
    """The Whitaker suite arrives as a verified release, not a compiled crate."""
    step = inv.find_step(_build_steps(), "Install Whitaker")
    assert inv.step_action(step) == (
        "leynos/shared-actions/.github/actions/install-whitaker"
    ), "Whitaker must come from the shared installer action"
    options = step.get("with")
    assert isinstance(options, dict), "the Whitaker installer needs inputs"
    assert options.get("cache-provider") == "external", (
        "the build job owns ~/.cargo/bin, so the action must not cache it too"
    )


def _is_global_tool_install(command: str) -> bool:
    """Report whether a command installs a Bun or uv tool outside the project."""
    is_bun_global = "bun install" in command and (
        "--global" in command or " -g " in command
    )
    return is_bun_global or "uv tool install" in command


def test_global_bun_and_uv_installs_are_version_pinned() -> None:
    """An unpinned global install makes the tool cache key unexplainable."""
    unpinned = [
        (filename, job_id, step_name, command)
        for filename, job_id, step_name, command in inv.iter_step_commands()
        if _is_global_tool_install(command)
        # Every pin is carried by a workflow-level environment variable so the
        # same value also feeds the tool cache key.
        if "@${" not in command and "==${" not in command
    ]
    assert unpinned == [], f"these global installs are not version-pinned: {unpinned}"


def test_no_source_build_remains_in_the_makefile() -> None:
    """Nothing in CI is compiled from source, including pg_worker.

    `cargo install --list` is a query rather than an installation, and the
    pg_worker pin probe uses it, so it is the one form allowed here.
    pg-embed-setup-unpriv published checksum-verified archives from v0.5.2 and
    the last source build went with them.
    """
    makefile = inv.MAKEFILE_PATH.read_text(encoding="utf-8")
    occurrences = [
        line.strip()
        for line in makefile.splitlines()
        if "cargo install" in line
        and "cargo install --list" not in line
        and not line.lstrip().startswith("#")
    ]
    assert occurrences == [], f"these lines build from source: {occurrences}"


def test_the_pg_worker_install_cannot_fall_back_to_compiling() -> None:
    """`cargo binstall` compiles unless the strategy list forbids it.

    The Makefile is outside the workflow-step scan that guards the same thing
    in CI, so it needs its own assertion rather than inheriting one. Line
    continuations are joined first: the strategy list sits on its own line, so
    a per-line check would report the invocation as unguarded.
    """
    makefile = inv.MAKEFILE_PATH.read_text(encoding="utf-8")
    installs = [
        line.strip()
        for line in makefile.replace("\\\n", " ").splitlines()
        if "cargo binstall" in line and not line.lstrip().startswith("#")
    ]
    assert installs, "the Makefile must install pg_worker from a release archive"
    assert all(BINSTALL_FAIL_CLOSED in line for line in installs), (
        f"these invocations must pass {BINSTALL_FAIL_CLOSED}: {installs}"
    )


def test_the_pg_worker_pin_is_probed_by_version_not_by_presence() -> None:
    """A binary restored under an older pin must be replaced, not reused.

    `pg_worker` has no `--version` flag, so cargo's install manifest is the
    only probe available. A `command -v` check would silently keep a stale
    binary, which is the failure the tool cache makes most likely.
    """
    makefile = inv.MAKEFILE_PATH.read_text(encoding="utf-8")
    assert "PG_EMBED_SETUP_UNPRIV_VERSION ?=" in makefile, (
        "the pg_worker pin must live in the Makefile"
    )
    assert 'grep -qx "pg-embed-setup-unpriv v$(PG_EMBED_SETUP_UNPRIV_VERSION):"' in (
        makefile
    ), "the probe must match the pinned version exactly, not merely the binary"
    # The manifest and the file live in different cache archives, so either can
    # arrive without the other. A manifest naming the pinned version with the
    # binary missing sends the copy to a path that is not there.
    assert '[ -x "$$pinned" ] &&' in makefile, (
        "the probe must confirm the binary exists, not only that cargo recorded it"
    )
    # binstall consults the same manifest the probe just rejected, so without
    # --force it is a no-op in exactly the case the probe exists to repair.
    # Match the invocation: `--force-exclude` elsewhere in the Makefile would
    # satisfy a search for the flag on its own.
    installs = [
        line
        for line in makefile.replace("\\\n", " ").splitlines()
        if "cargo binstall" in line and not line.lstrip().startswith("#")
    ]
    assert installs and all("--force" in line for line in installs), (
        "the reinstall must override the manifest the probe rejected"
    )


def test_the_cargo_install_manifest_travels_with_the_binaries() -> None:
    """The pg_worker probe reads the manifest, so it must be cached with them.

    An archive that restored `~/.cargo/bin` without `~/.cargo/.crates2.json`
    would bring back the binary and lose the record of its version, so the
    probe would miss and reinstall on every warm run. The failure is silent:
    the job stays green and merely does the work again.
    """
    offenders = [
        (filename, job_id, step.get("name"))
        for filename, job_id, step in _cache_steps_with_paths()
        if "~/.cargo/bin" in inv.cache_paths(step)
        if "~/.cargo/.crates2.json" not in inv.cache_paths(step)
    ]
    assert offenders == [], (
        f"these archives carry the cargo bin directory without its manifest: {offenders}"
    )


def test_the_pg_worker_install_is_authenticated() -> None:
    """An unauthenticated release lookup is rate-limited hard.

    `cargo binstall` resolves the archive through the GitHub API, so the step
    needs the token for the same reason the PostgreSQL warm-up does.
    """
    for filename, job_id, job in inv.iter_jobs():
        for step in inv.job_steps(job):
            if step.get("name") != "Install pg_worker binary":
                continue
            env = step.get("env")
            assert isinstance(env, dict), (
                f"{filename}:{job_id} pg_worker install needs a token"
            )
            assert "GITHUB_TOKEN" in env, (
                f"{filename}:{job_id} must authenticate the release lookup"
            )


@pytest.mark.parametrize(("installer", "first_use"), BUILD_INSTALLER_ORDER)
def test_required_tool_setup_precedes_first_use(installer: str, first_use: str) -> None:
    """A gate that runs before its installer fails for the wrong reason."""
    steps = _build_steps()
    assert inv.step_index(steps, installer) < inv.step_index(steps, first_use), (
        f"{installer!r} must precede {first_use!r}"
    )


@pytest.mark.parametrize(("filename", "job_id"), GITHUB_HOSTED_JOBS)
def test_non_build_jobs_stay_github_hosted(filename: str, job_id: str) -> None:
    """Scheduled and API-bound work keeps its GitHub-hosted placement."""
    job = inv.load_workflow(filename)["jobs"][job_id]
    assert job.get("runs-on") in inv.GITHUB_HOSTED_LABELS, (
        f"{filename}:{job_id} must stay on a GitHub-hosted runner"
    )


@pytest.mark.parametrize(("filename", "job_id"), REUSABLE_CALLER_JOBS)
def test_reusable_callers_do_not_select_a_runner(filename: str, job_id: str) -> None:
    """The callee owns the runner, so the caller must not name one."""
    job = inv.load_workflow(filename)["jobs"][job_id]
    assert "runs-on" not in job, f"{filename}:{job_id} must not override the runner"
    assert isinstance(job.get("uses"), str), (
        f"{filename}:{job_id} must call a reusable workflow"
    )


@pytest.mark.parametrize(("filename", "job_id"), sorted(inv.BUILD_JOBS))
def test_build_jobs_keep_their_reviewed_label_and_a_timeout(
    filename: str, job_id: str
) -> None:
    """A job with no timeout can hold a managed runner until the platform cap."""
    job = inv.load_workflow(filename)["jobs"][job_id]
    assert job.get("runs-on") in inv.MANAGED_RUNNER_LABELS, (
        f"{filename}:{job_id} must keep its reviewed managed-runner label"
    )
    assert isinstance(job.get("timeout-minutes"), int), (
        f"{filename}:{job_id} must declare timeout-minutes"
    )


def test_the_coverage_tool_cache_key_tracks_the_shared_action_pin() -> None:
    """The coverage lane's executables are chosen by the pinned action.

    Its commit is therefore the only honest key for the archive holding them:
    a bump that left the key unchanged would restore the previous action's
    binaries. This assertion is a deliberate lockstep with the pin, and a
    Dependabot bump is expected to fail it until the key is refreshed.
    """
    refs = {reference[2] for reference in _shared_action_references()}
    assert len(refs) == 1, f"the estate spans several shared-actions commits: {refs}"
    pin = refs.pop()
    for filename in ("ci.yml", "coverage-main.yml"):
        assert f"SHARED_ACTIONS_PIN: {pin}" in _workflow_text(filename), (
            f"{filename} must key its coverage tool cache by the action pin"
        )
