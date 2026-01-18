#!/usr/bin/env -S uv run python
# /// script
# requires-python = ">=3.13"
# dependencies = ["cyclopts>=2.9"]
# ///
"""Commit rendered manifests to the GitOps repository.

This script clones the GitOps repository, replaces the cluster manifest
directory with rendered output, commits and pushes changes, and exports
the commit SHA to ``GITHUB_ENV``.

Examples
--------
>>> python scripts/commit_gitops_manifests.py --gitops-repository org/repo
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
import tempfile
from collections.abc import Iterator
from contextlib import contextmanager
from dataclasses import dataclass
from pathlib import Path

from cyclopts import App, Parameter
from scripts._input_resolution import InputResolution, resolve_input
from scripts._infra_k8s import append_github_env, mask_secret, parse_bool

app = App(help="Commit rendered manifests to the GitOps repository.")

GITOPS_REPOSITORY_PARAM = Parameter()
GITOPS_BRANCH_PARAM = Parameter()
GITOPS_TOKEN_PARAM = Parameter()
CLUSTER_NAME_PARAM = Parameter()
RENDER_OUTPUT_DIR_PARAM = Parameter()
RUNNER_TEMP_PARAM = Parameter()
GITHUB_ENV_PARAM = Parameter()
DRY_RUN_PARAM = Parameter()


class GitOpsError(Exception):
    """Base error for GitOps manifest operations."""


class GitCommandError(GitOpsError):
    """Raised when a git command fails."""


class GitCloneError(GitOpsError):
    """Raised when cloning the GitOps repository fails."""


class GitValidationError(GitOpsError):
    """Raised when GitOps paths are unsafe or invalid."""


class GitSyncError(GitOpsError):
    """Raised when syncing manifests fails."""


@dataclass(frozen=True, slots=True)
class GitOpsInputs:
    """Inputs for GitOps commit.

    Attributes
    ----------
    gitops_repository, gitops_branch, gitops_token : str
        GitOps repository configuration values.
    cluster_name : str
        Cluster name used for manifest paths.
    render_output_dir, runner_temp, github_env : Path
        Paths for rendered manifests, working directories, and GITHUB_ENV.
    dry_run : bool
        Whether to skip the git push step.
    """

    # GitOps configuration
    gitops_repository: str
    gitops_branch: str
    gitops_token: str

    # Cluster identification
    cluster_name: str

    # Paths
    render_output_dir: Path
    runner_temp: Path
    github_env: Path

    # Options
    dry_run: bool


@dataclass(frozen=True, slots=True)
class RawGitOpsInputs:
    """Raw GitOps inputs from CLI or defaults.

    Attributes
    ----------
    gitops_repository, gitops_branch, gitops_token : str | None
        Raw GitOps repository inputs.
    cluster_name : str | None
        Raw cluster name input.
    render_output_dir, runner_temp, github_env : Path | None
        Raw path overrides.
    dry_run : str | None
        Raw dry-run flag.
    """

    gitops_repository: str | None = None
    gitops_branch: str | None = None
    gitops_token: str | None = None
    cluster_name: str | None = None
    render_output_dir: Path | None = None
    runner_temp: Path | None = None
    github_env: Path | None = None
    dry_run: str | None = None


def _to_path(value: Path | str | None) -> Path:
    """Coerce a string or Path to Path."""
    if value is None:
        msg = "Path value must not be None"
        raise GitValidationError(msg)
    return value if isinstance(value, Path) else Path(value)


def resolve_gitops_inputs(raw: RawGitOpsInputs) -> GitOpsInputs:
    """Resolve GitOps inputs from environment.

    Parameters
    ----------
    raw : RawGitOpsInputs
        Raw inputs from CLI or defaults.

    Returns
    -------
    GitOpsInputs
        Normalized inputs for GitOps operations.

    Examples
    --------
    >>> resolve_gitops_inputs(RawGitOpsInputs())
    """
    gitops_repository = resolve_input(
        raw.gitops_repository, InputResolution(env_key="GITOPS_REPOSITORY", required=True)
    )
    gitops_branch = resolve_input(
        raw.gitops_branch, InputResolution(env_key="GITOPS_BRANCH", default="main")
    )
    gitops_token = resolve_input(
        raw.gitops_token, InputResolution(env_key="GITOPS_TOKEN", required=True)
    )

    cluster_name = resolve_input(
        raw.cluster_name, InputResolution(env_key="CLUSTER_NAME", required=True)
    )

    render_output_dir_raw = resolve_input(
        raw.render_output_dir,
        InputResolution(
            env_key="RENDER_OUTPUT_DIR",
            default=Path("/tmp/rendered-manifests"),
            as_path=True,
        ),
    )
    runner_temp_raw = resolve_input(
        raw.runner_temp,
        InputResolution(env_key="RUNNER_TEMP", default=Path("/tmp"), as_path=True),
    )
    github_env_raw = resolve_input(
        raw.github_env,
        InputResolution(
            env_key="GITHUB_ENV",
            default=Path("/tmp/github-env-undefined"),
            as_path=True,
        ),
    )
    dry_run_raw = resolve_input(
        raw.dry_run, InputResolution(env_key="DRY_RUN", default="false")
    )

    return GitOpsInputs(
        gitops_repository=str(gitops_repository),
        gitops_branch=str(gitops_branch),
        gitops_token=str(gitops_token),
        cluster_name=str(cluster_name),
        render_output_dir=_to_path(render_output_dir_raw),
        runner_temp=_to_path(runner_temp_raw),
        github_env=_to_path(github_env_raw),
        dry_run=parse_bool(str(dry_run_raw) if dry_run_raw else None, default=False),
    )


def run_git(args: list[str], cwd: Path, env: dict[str, str] | None = None) -> str:
    """Run a git command and return stdout.

    Parameters
    ----------
    args : list[str]
        Git arguments (without the ``git`` prefix).
    cwd : Path
        Directory to run the command in.
    env : dict[str, str] | None, optional
        Environment overrides for the command.

    Returns
    -------
    str
        Stdout from the git command.

    Raises
    ------
    GitCommandError
        Raised when the git command fails.
    """
    merged_env = {**os.environ, **(env or {})}
    result = subprocess.run(
        ["git", *args],
        cwd=cwd,
        env=merged_env,
        capture_output=True,
        text=True,
        check=False,
    )

    if result.returncode != 0:
        msg = f"git {' '.join(args)} failed: {result.stderr}"
        raise GitCommandError(msg)

    return result.stdout.strip()


@contextmanager
def _git_auth_env(token: str, base_dir: Path) -> Iterator[dict[str, str]]:
    """Build environment for Git askpass authentication."""
    base_dir.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(
        mode="w",
        delete=False,
        prefix="git-askpass-",
        dir=base_dir,
        encoding="utf-8",
    ) as handle:
        handle.write("#!/bin/sh\nprintf '%s' \"${GITOPS_TOKEN}\"\n")
        askpass_path = Path(handle.name)
    askpass_path.chmod(0o700)
    env = {
        **os.environ,
        "GIT_ASKPASS": str(askpass_path),
        "GIT_TERMINAL_PROMPT": "0",
        "GITOPS_TOKEN": token,
    }
    try:
        yield env
    finally:
        askpass_path.unlink(missing_ok=True)


def clone_repository(
    inputs: GitOpsInputs,
    clone_dir: Path,
    auth_env: dict[str, str],
) -> None:
    """Clone the GitOps repository.

    Parameters
    ----------
    inputs : GitOpsInputs
        Normalized GitOps inputs.
    clone_dir : Path
        Directory for the clone.
    auth_env : dict[str, str]
        Environment containing Git authentication configuration.

    Raises
    ------
    GitCloneError
        Raised when the clone fails.
    """
    mask_secret(inputs.gitops_token)
    repo_url = f"https://x-access-token@github.com/{inputs.gitops_repository}.git"
    masked_repo_url = f"https://x-access-token:***@github.com/{inputs.gitops_repository}.git"

    print(f"Cloning {inputs.gitops_repository}@{inputs.gitops_branch}...")

    # Clone with depth 1 for efficiency
    result = subprocess.run(
        [
            "git",
            "clone",
            "--depth",
            "1",
            "--branch",
            inputs.gitops_branch,
            repo_url,
            str(clone_dir),
        ],
        check=False,
        capture_output=True,
        text=True,
        env=auth_env,
    )
    if result.returncode != 0:
        stderr = result.stderr.replace(inputs.gitops_token, "***")
        msg = f"git clone failed for {masked_repo_url}: {stderr}".strip()
        raise GitCloneError(msg)


def sync_manifests(
    inputs: GitOpsInputs,
    clone_dir: Path,
) -> int:
    """Sync rendered manifests to the GitOps repository.

    Parameters
    ----------
    inputs : GitOpsInputs
        Normalized GitOps inputs.
    clone_dir : Path
        Path to the cloned repository.

    Returns
    -------
    int
        Number of files synced.
    """
    cluster_root = clone_dir / "clusters"
    cluster_dir = cluster_root / inputs.cluster_name
    _reset_cluster_dir(cluster_root, cluster_dir)

    count = 0

    # Copy all rendered manifests to the cluster directory
    if inputs.render_output_dir.exists():
        for src_file in inputs.render_output_dir.rglob("*"):
            if src_file.is_file():
                rel_path = src_file.relative_to(inputs.render_output_dir)
                dest_file = cluster_dir / rel_path
                dest_file.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(src_file, dest_file)
                count += 1

    return count


def _reset_cluster_dir(cluster_root: Path, cluster_dir: Path) -> None:
    """Ensure the cluster directory is empty and safe to reuse."""
    cluster_root_resolved = cluster_root.resolve()
    clone_dir_resolved = cluster_root.parent.resolve()
    cluster_dir_resolved = cluster_dir.resolve()

    if cluster_root.is_symlink():
        msg = f"Refusing to reset symlinked cluster root {cluster_root}"
        raise GitValidationError(msg)
    if not cluster_root_resolved.is_relative_to(clone_dir_resolved):
        msg = f"Refusing to reset cluster root outside {clone_dir_resolved}"
        raise GitValidationError(msg)
    if not cluster_dir_resolved.is_relative_to(cluster_root_resolved):
        msg = f"Refusing to reset cluster dir outside {cluster_root_resolved}"
        raise GitValidationError(msg)

    if cluster_dir.exists():
        shutil.rmtree(cluster_dir)
    cluster_dir.mkdir(parents=True, exist_ok=True)


def commit_and_push(
    inputs: GitOpsInputs,
    clone_dir: Path,
    auth_env: dict[str, str],
) -> str | None:
    """Commit and push changes to the GitOps repository.

    Parameters
    ----------
    inputs : GitOpsInputs
        Normalized GitOps inputs.
    clone_dir : Path
        Path to the cloned repository.
    auth_env : dict[str, str]
        Environment containing Git authentication configuration.

    Returns
    -------
    str | None
        Commit SHA if changes were pushed, otherwise ``None``.
    """
    # Configure git user for the commit
    run_git(["config", "user.name", "wildside-infra-k8s-action"], clone_dir)
    run_git(["config", "user.email", "actions@wildside.dev"], clone_dir)

    # Stage all changes
    run_git(["add", "-A"], clone_dir)

    # Check for changes
    result = subprocess.run(
        ["git", "diff", "--cached", "--quiet"],
        cwd=clone_dir,
        capture_output=True,
        check=False,
    )

    if result.returncode == 0:
        print("No changes to commit")
        return None

    # Commit changes
    commit_message = f"Update manifests for cluster {inputs.cluster_name}"
    run_git(["commit", "-m", commit_message], clone_dir)

    # Get commit SHA
    commit_sha = run_git(["rev-parse", "HEAD"], clone_dir)

    if inputs.dry_run:
        print(f"Dry run mode - would push commit {commit_sha}")
        return commit_sha

    # Push changes
    print(f"Pushing commit {commit_sha}...")
    run_git(["push", "origin", inputs.gitops_branch], clone_dir, env=auth_env)

    return commit_sha


# CLI parameters are declared for cyclopts but resolved via resolve_gitops_inputs().
# This keeps defaults centralized while preventing ARG001/B008 false positives.
@app.command()
def main(
    gitops_repository: str | None = GITOPS_REPOSITORY_PARAM,
    gitops_branch: str | None = GITOPS_BRANCH_PARAM,
    gitops_token: str | None = GITOPS_TOKEN_PARAM,
    cluster_name: str | None = CLUSTER_NAME_PARAM,
    render_output_dir: Path | None = RENDER_OUTPUT_DIR_PARAM,
    runner_temp: Path | None = RUNNER_TEMP_PARAM,
    github_env: Path | None = GITHUB_ENV_PARAM,
    dry_run: str | None = DRY_RUN_PARAM,
) -> int:
    """Commit rendered manifests to the GitOps repository.

    Parameters
    ----------
    gitops_repository, gitops_branch, gitops_token : str | None
        CLI overrides for GitOps repository configuration.
    cluster_name : str | None
        Cluster name override for the destination directory.
    render_output_dir, runner_temp, github_env : Path | None
        Path overrides for rendered manifests, scratch space, and GITHUB_ENV.
    dry_run : str | None
        Dry-run override to skip pushes.

    Returns
    -------
    int
        Exit code (0 for success).

    Examples
    --------
    >>> python scripts/commit_gitops_manifests.py --gitops-repository org/repo
    """
    raw_inputs = RawGitOpsInputs(
        gitops_repository=gitops_repository,
        gitops_branch=gitops_branch,
        gitops_token=gitops_token,
        cluster_name=cluster_name,
        render_output_dir=render_output_dir,
        runner_temp=runner_temp,
        github_env=github_env,
        dry_run=dry_run,
    )
    # Resolve inputs from environment
    inputs = resolve_gitops_inputs(raw_inputs)

    # Mask sensitive values
    mask_secret(inputs.gitops_token)

    print(f"Committing manifests for cluster '{inputs.cluster_name}'...")
    print(f"  Repository: {inputs.gitops_repository}")
    print(f"  Branch: {inputs.gitops_branch}")
    print(f"  Source: {inputs.render_output_dir}")
    print(f"  Dry run: {inputs.dry_run}")

    # Create clone directory
    clone_dir = inputs.runner_temp / "gitops-clone"
    if clone_dir.exists():
        shutil.rmtree(clone_dir)
    clone_dir.mkdir(parents=True)

    try:
        with _git_auth_env(inputs.gitops_token, inputs.runner_temp) as auth_env:
            # Clone the repository
            clone_repository(inputs, clone_dir, auth_env)

            # Sync manifests
            count = sync_manifests(inputs, clone_dir)
            print(f"Synced {count} manifest files")

            commit_sha = None
            if count > 0:
                # Commit and push
                commit_sha = commit_and_push(inputs, clone_dir, auth_env)
    except GitOpsError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1
    else:
        if count == 0:
            print("No manifests to commit")
            return 0

        # Export commit SHA to GITHUB_ENV
        if commit_sha:
            append_github_env(
                inputs.github_env,
                {"GITOPS_COMMIT_SHA": commit_sha},
            )
            print(f"\nCommit SHA: {commit_sha}")

        print("\nGitOps commit complete.")
        return 0


if __name__ == "__main__":  # pragma: no cover - CLI entrypoint
    raise SystemExit(app())
