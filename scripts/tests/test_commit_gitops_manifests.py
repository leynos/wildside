"""Unit tests for commit_gitops_manifests."""

from __future__ import annotations

import sys
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from scripts.commit_gitops_manifests import (  # noqa: E402
    GitOpsInputs,
    RawGitOpsInputs,
    _git_auth_env,
    clone_repository,
    commit_and_push,
    resolve_gitops_inputs,
    sync_manifests,
)


def _make_inputs(tmp_path: Path, **overrides: object) -> GitOpsInputs:
    defaults: dict[str, object] = {
        "gitops_repository": "wildside/wildside-infra",
        "gitops_branch": "main",
        "gitops_token": "token",
        "cluster_name": "preview-1",
        "render_output_dir": tmp_path / "render",
        "runner_temp": tmp_path,
        "github_env": tmp_path / "env",
        "dry_run": False,
    }
    defaults.update(overrides)
    return GitOpsInputs(**defaults)


def test_resolve_gitops_inputs_cli_override(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    monkeypatch.setenv("GITOPS_REPOSITORY", "wildside/env")
    monkeypatch.setenv("GITOPS_TOKEN", "token")
    monkeypatch.setenv("CLUSTER_NAME", "env")
    monkeypatch.setenv("RUNNER_TEMP", str(tmp_path))
    monkeypatch.setenv("GITHUB_ENV", str(tmp_path / "env"))

    inputs = resolve_gitops_inputs(RawGitOpsInputs(gitops_repository="cli"))
    assert inputs.gitops_repository == "cli"


def test_clone_repository_uses_askpass(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    inputs = _make_inputs(tmp_path, gitops_token="super-secret")
    clone_dir = tmp_path / "clone"
    clone_dir.mkdir()

    captured: dict[str, object] = {}

    class _Result:
        def __init__(self) -> None:
            self.returncode = 0
            self.stderr = ""

    def fake_run(cmd: list[str], **kwargs: object) -> _Result:  # type: ignore[override]
        captured["cmd"] = cmd
        captured["env"] = kwargs.get("env")
        return _Result()

    monkeypatch.setattr("scripts.commit_gitops_manifests.subprocess.run", fake_run)

    auth_env = _git_auth_env(inputs.gitops_token, tmp_path)
    clone_repository(inputs, clone_dir, auth_env)

    cmd = captured["cmd"]
    assert "super-secret" not in " ".join(cmd)
    env = captured["env"]
    assert isinstance(env, dict)
    assert env.get("GITOPS_TOKEN") == "super-secret"
    assert env.get("GIT_ASKPASS")

    askpass_path = Path(env["GIT_ASKPASS"])
    assert askpass_path.exists()
    assert "super-secret" not in askpass_path.read_text(encoding="utf-8")


def test_sync_manifests_copies_files(tmp_path: Path) -> None:
    inputs = _make_inputs(tmp_path)
    inputs.render_output_dir.mkdir(parents=True)
    (inputs.render_output_dir / "platform").mkdir()
    source = inputs.render_output_dir / "platform" / "manifest.yaml"
    source.write_text("apiVersion: v1", encoding="utf-8")

    clone_dir = tmp_path / "clone"
    clone_dir.mkdir()

    count = sync_manifests(inputs, clone_dir)
    assert count == 1
    dest = clone_dir / "clusters" / inputs.cluster_name / "platform" / "manifest.yaml"
    assert dest.read_text(encoding="utf-8") == "apiVersion: v1"


def test_sync_manifests_removes_stale_files(tmp_path: Path) -> None:
    inputs = _make_inputs(tmp_path)
    inputs.render_output_dir.mkdir(parents=True)
    (inputs.render_output_dir / "platform").mkdir()
    (inputs.render_output_dir / "platform" / "manifest.yaml").write_text(
        "apiVersion: v1",
        encoding="utf-8",
    )

    clone_dir = tmp_path / "clone"
    cluster_dir = clone_dir / "clusters" / inputs.cluster_name / "old"
    cluster_dir.mkdir(parents=True)
    stale_manifest = cluster_dir / "stale.yaml"
    stale_manifest.write_text("stale", encoding="utf-8")

    sync_manifests(inputs, clone_dir)

    assert not stale_manifest.exists()


def test_commit_and_push_no_changes(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    inputs = _make_inputs(tmp_path)
    clone_dir = tmp_path / "clone"
    clone_dir.mkdir()

    monkeypatch.setattr(
        "scripts.commit_gitops_manifests.run_git",
        lambda *_args, **_kwargs: "",
    )

    class _Result:
        def __init__(self) -> None:
            self.returncode = 0

    monkeypatch.setattr(
        "scripts.commit_gitops_manifests.subprocess.run",
        lambda *_args, **_kwargs: _Result(),
    )

    auth_env = _git_auth_env(inputs.gitops_token, tmp_path)
    assert commit_and_push(inputs, clone_dir, auth_env) is None


def test_commit_and_push_dry_run(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    inputs = _make_inputs(tmp_path, dry_run=True)
    clone_dir = tmp_path / "clone"
    clone_dir.mkdir()

    calls: list[list[str]] = []

    def fake_run_git(args: list[str], _cwd: Path, env: dict[str, str] | None = None) -> str:
        calls.append(args)
        if args[:2] == ["rev-parse", "HEAD"]:
            return "abc123"
        return ""

    monkeypatch.setattr("scripts.commit_gitops_manifests.run_git", fake_run_git)

    class _Result:
        def __init__(self) -> None:
            self.returncode = 1

    monkeypatch.setattr(
        "scripts.commit_gitops_manifests.subprocess.run",
        lambda *_args, **_kwargs: _Result(),
    )

    auth_env = _git_auth_env(inputs.gitops_token, tmp_path)
    commit_sha = commit_and_push(inputs, clone_dir, auth_env)
    assert commit_sha == "abc123"
    assert ["push", "origin", inputs.gitops_branch] not in calls
