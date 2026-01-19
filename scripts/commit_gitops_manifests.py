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

import sys
import tempfile
from pathlib import Path

from cyclopts import App, Parameter
from scripts._gitops_errors import GitOpsError, GitValidationError
from scripts._gitops_inputs import GitOpsInputs, RawGitOpsInputs, resolve_gitops_inputs
from scripts._gitops_repo import (
    _git_auth_env,
    clone_repository,
    commit_and_push,
    sync_manifests,
)
from scripts._infra_k8s import append_github_env, mask_secret

app = App(help="Commit rendered manifests to the GitOps repository.")

GITOPS_REPOSITORY_PARAM = Parameter()
GITOPS_BRANCH_PARAM = Parameter()
GITOPS_TOKEN_PARAM = Parameter()
CLUSTER_NAME_PARAM = Parameter()
RENDER_OUTPUT_DIR_PARAM = Parameter()
RUNNER_TEMP_PARAM = Parameter()
GITHUB_ENV_PARAM = Parameter()
DRY_RUN_PARAM = Parameter()
# CLI parameters are declared for cyclopts but resolved via resolve_gitops_inputs().
# This keeps defaults centralised while preventing ARG001/B008 false positives.
def _build_raw_inputs_from_cli(
    gitops_repository: str | None,
    gitops_branch: str | None,
    gitops_token: str | None,
    cluster_name: str | None,
    render_output_dir: Path | None,
    runner_temp: Path | None,
    github_env: Path | None,
    dry_run: str | None,
) -> RawGitOpsInputs:
    """Build raw GitOps inputs from CLI arguments."""
    return RawGitOpsInputs(
        gitops_repository=gitops_repository,
        gitops_branch=gitops_branch,
        gitops_token=gitops_token,
        cluster_name=cluster_name,
        render_output_dir=render_output_dir,
        runner_temp=runner_temp,
        github_env=github_env,
        dry_run=dry_run,
    )


def _run_gitops_flow(inputs: GitOpsInputs) -> tuple[int, str | None]:
    """Clone, sync, and commit GitOps manifests."""
    if inputs.runner_temp.exists() and not inputs.runner_temp.is_dir():
        msg = f"RUNNER_TEMP must be a directory: {inputs.runner_temp}"
        raise GitValidationError(msg)
    inputs.runner_temp.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=inputs.runner_temp) as tmp_dir:
        base_dir = Path(tmp_dir)
        clone_dir = base_dir / "repo"
        with _git_auth_env(inputs.gitops_token, base_dir) as auth_env:
            clone_repository(inputs, clone_dir, auth_env)
            synced = sync_manifests(inputs, clone_dir)
            commit_sha = None
            if synced > 0:
                commit_sha = commit_and_push(inputs, clone_dir, auth_env)
    return synced, commit_sha


def _export_commit_sha(github_env: Path, commit_sha: str | None) -> None:
    """Export the GitOps commit SHA to GITHUB_ENV when available."""
    if commit_sha:
        append_github_env(github_env, {"GITOPS_COMMIT_SHA": commit_sha})


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
    raw_inputs = _build_raw_inputs_from_cli(
        gitops_repository,
        gitops_branch,
        gitops_token,
        cluster_name,
        render_output_dir,
        runner_temp,
        github_env,
        dry_run,
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

    try:
        synced, commit_sha = _run_gitops_flow(inputs)
        print(f"Synced {synced} manifest files")
    except GitOpsError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1
    else:
        if synced == 0:
            print("No manifests to commit")
            return 0

        # Export commit SHA to GITHUB_ENV
        _export_commit_sha(inputs.github_env, commit_sha)
        if commit_sha:
            print(f"\nCommit SHA: {commit_sha}")

        print("\nGitOps commit complete.")
        return 0


if __name__ == "__main__":  # pragma: no cover - CLI entrypoint
    raise SystemExit(app())
