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
    with, so the archive costs transfer time on nearly every run. There is no
    longer an exception: `target/pg-worker-root` went when pg_worker stopped
    being built from source.
    """
    offenders = [
        (filename, job_id, step.get("name"), path)
        for filename, job_id, step in _cache_steps()
        for path in inv.cache_paths(step)
        if path == "target" or path.startswith("target/")
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
