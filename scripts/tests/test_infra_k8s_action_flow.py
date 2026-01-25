"""Behavioural test for wildside-infra-k8s action flow."""

from __future__ import annotations

import inspect
import secrets
from dataclasses import dataclass
from collections.abc import Callable
from pathlib import Path

import pytest

from scripts._infra_k8s import TofuResult
from scripts.commit_gitops_manifests import main as commit_gitops_main
from scripts.prepare_infra_k8s_inputs import main as prepare_inputs_main
from scripts.provision_cluster import main as provision_cluster_main
from scripts.publish_infra_k8s_outputs import main as publish_outputs_main
from scripts.render_platform_manifests import main as render_manifests_main

EnvSetter = Callable[[str, str], None]


@dataclass(frozen=True, slots=True)
class FlowPaths:
    """Paths used by the end-to-end action flow."""

    runner_temp: Path
    github_env: Path
    github_output: Path
    render_output_dir: Path
    clone_dir: Path


def _is_blank_or_comment(line: str) -> bool:
    """Return True when a line is empty or a comment."""
    return not line.strip() or line.lstrip().startswith("#")


def _start_heredoc(line: str) -> tuple[str | None, str | None]:
    """Detect a heredoc start and return the key and marker."""
    if "<<" not in line:
        return None, None
    key_part, marker = line.split("<<", 1)
    return key_part, marker


def _flush_heredoc(entries: dict[str, str], key: str | None, buf: list[str]) -> None:
    """Store buffered heredoc content in entries when a key is present."""
    if key is None:
        return
    entries[key] = "\n".join(buf)


def _heredoc_step(
    delimiter: str | None,
    key: str | None,
    buffer: list[str],
    line: str,
    entries: dict[str, str],
) -> tuple[str | None, str | None, list[str], bool]:
    if delimiter is None:
        return key, delimiter, buffer, False
    if line == delimiter:
        _flush_heredoc(entries, key, buffer)
        return None, None, [], True
    buffer.append(line)
    return key, delimiter, buffer, True


def _parse_github_kv_file(path: Path) -> dict[str, str]:
    """Parse a GitHub-style KEY=VALUE file with heredoc support."""
    if not path.exists():
        return {}
    entries: dict[str, str] = {}
    key: str | None = None
    delimiter: str | None = None
    buffer: list[str] = []
    with path.open("r", encoding="utf-8") as handle:
        for raw_line in handle:
            line = raw_line.rstrip("\n")
            key, delimiter, buffer, consumed = _heredoc_step(
                delimiter,
                key,
                buffer,
                line,
                entries,
            )
            if consumed:
                continue
            if _is_blank_or_comment(line):
                continue
            next_key, next_delim = _start_heredoc(line)
            if next_key is not None and next_delim is not None:
                key = next_key
                delimiter = next_delim
                buffer = []
                continue
            if "=" in line:
                key_part, _, value = line.partition("=")
                entries[key_part.strip()] = value
    if delimiter is not None:
        _flush_heredoc(entries, key, buffer)
    return entries


def _apply_env_file(env_path: Path, setenv: EnvSetter) -> None:
    """Apply GITHUB_ENV entries to the process environment."""
    values = _parse_github_kv_file(env_path)
    for key, value in values.items():
        setenv(key, value)


def _mk_paths(tmp_path: Path) -> FlowPaths:
    """Create the paths used by the action flow."""
    runner_temp = tmp_path / "runner"
    render_output_dir = tmp_path / "rendered"
    github_env = tmp_path / "github-env"
    github_output = tmp_path / "github-output"
    clone_dir = runner_temp / "gitops-clone"
    runner_temp.mkdir(parents=True)
    render_output_dir.mkdir(parents=True)

    return FlowPaths(
        runner_temp=runner_temp,
        github_env=github_env,
        github_output=github_output,
        render_output_dir=render_output_dir,
        clone_dir=clone_dir,
    )


def _set_base_env(monkeypatch: pytest.MonkeyPatch, paths: FlowPaths) -> None:
    """Seed baseline environment variables for the action flow."""
    token = _dummy_token()
    env_vars = {
        "INPUT_CLUSTER_NAME": "preview-1",
        "INPUT_ENVIRONMENT": "preview",
        "INPUT_REGION": "nyc1",
        "INPUT_DOMAIN": "example.test",
        "INPUT_ACME_EMAIL": "admin@example.test",
        "INPUT_NODE_POOLS": "[]",
        "INPUT_GITOPS_REPOSITORY": "wildside/wildside-infra",
        "INPUT_GITOPS_TOKEN": token,
        "INPUT_VAULT_ADDRESS": "https://vault.example.test:8200",
        "INPUT_VAULT_ROLE_ID": _dummy_token(),
        "INPUT_VAULT_SECRET_ID": _dummy_token(),
        "INPUT_DIGITALOCEAN_TOKEN": _dummy_token(),
        "INPUT_SPACES_ACCESS_KEY": _dummy_token(),
        "INPUT_SPACES_SECRET_KEY": _dummy_token(),
        "INPUT_DRY_RUN": "true",
        "RUNNER_TEMP": str(paths.runner_temp),
        "GITHUB_ENV": str(paths.github_env),
        "GITHUB_OUTPUT": str(paths.github_output),
        "RENDER_OUTPUT_DIR": str(paths.render_output_dir),
    }
    for key, value in env_vars.items():
        monkeypatch.setenv(key, value)


def _call_cli(main_func: Callable[..., object]) -> None:
    """Call a CLI entrypoint with explicit None overrides."""
    params = dict.fromkeys(inspect.signature(main_func).parameters, None)
    main_func(**params)


@pytest.fixture
def fake_tofu(monkeypatch: pytest.MonkeyPatch) -> None:
    """Stub OpenTofu interactions for the action flow."""
    tofu_result = TofuResult(
        success=True,
        stdout="",
        stderr="",
        return_code=0,
    )
    tofu_outputs = {
        "cluster_id": {"value": "abc"},
        "endpoint": {"value": "https://kube"},
        "kubeconfig": {"value": "kubeconfig"},
        "rendered_manifests": {"value": {"platform/traefik.yaml": "apiVersion: v1"}},
    }

    monkeypatch.setattr(
        "scripts._infra_k8s.run_tofu",
        lambda *_args, **_kwargs: tofu_result,
    )
    monkeypatch.setattr(
        "scripts._infra_k8s.tofu_output",
        lambda *_args, **_kwargs: tofu_outputs,
    )
    monkeypatch.setattr(
        "scripts.render_platform_manifests.run_tofu",
        lambda *_args, **_kwargs: tofu_result,
    )
    monkeypatch.setattr(
        "scripts.render_platform_manifests.tofu_output",
        lambda *_args, **_kwargs: tofu_outputs,
    )


@pytest.fixture
def fake_gitops(monkeypatch: pytest.MonkeyPatch) -> None:
    """Stub Git operations during the action flow."""
    monkeypatch.setattr(
        "scripts.commit_gitops_manifests.clone_repository",
        lambda *_args, **_kwargs: None,
    )
    monkeypatch.setattr(
        "scripts.commit_gitops_manifests.commit_and_push",
        lambda *_args, **_kwargs: "sha",
    )


def _parse_github_output(output_path: Path) -> dict[str, str]:
    """Parse the GITHUB_OUTPUT file into a normalized mapping."""
    outputs = _parse_github_kv_file(output_path)
    return {key.upper(): value for key, value in outputs.items()}


def _run_full_flow(paths: FlowPaths, setenv: EnvSetter) -> dict[str, str]:
    """Run the CLI entrypoints and return published outputs."""
    _call_cli(prepare_inputs_main)
    _apply_env_file(paths.github_env, setenv)
    _call_cli(provision_cluster_main)
    _apply_env_file(paths.github_env, setenv)
    _call_cli(render_manifests_main)
    _apply_env_file(paths.github_env, setenv)
    _call_cli(commit_gitops_main)
    _apply_env_file(paths.github_env, setenv)
    _call_cli(publish_outputs_main)
    return _parse_github_output(paths.github_output)


def _assert_published(outputs: dict[str, str]) -> None:
    """Assert the action publishes the expected outputs."""
    assert outputs["CLUSTER_NAME"] == "preview-1", "Cluster name should be published"
    assert (
        "RENDERED_MANIFEST_COUNT" in outputs
    ), "Rendered manifest count should be published"
    if commit_sha := outputs.get("GITOPS_COMMIT_SHA"):
        assert commit_sha, "Commit SHA should be non-empty when present"


def _dummy_token() -> str:
    """Return a random token-like value for tests."""
    return f"token-{secrets.token_hex(8)}"


@pytest.mark.usefixtures("fake_tofu", "fake_gitops")
def test_action_flow_happy_path(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    paths = _mk_paths(tmp_path)
    _set_base_env(monkeypatch, paths)
    outputs = _run_full_flow(paths, monkeypatch.setenv)
    _assert_published(outputs)
