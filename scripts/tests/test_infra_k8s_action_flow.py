"""Behavioural test for wildside-infra-k8s action flow."""

from __future__ import annotations

from pathlib import Path

import pytest

from scripts._infra_k8s import TofuResult, append_github_env, write_manifests
from scripts._provision_cluster_flow import export_cluster_outputs, provision_cluster
from scripts._provision_cluster_inputs import (
    RawProvisionInputs,
    build_backend_config,
    build_tfvars as build_cluster_tfvars,
    resolve_provision_inputs,
)
from scripts.commit_gitops_manifests import (
    RawGitOpsInputs,
    resolve_gitops_inputs,
    sync_manifests,
)
from scripts.prepare_infra_k8s_inputs import RawInputs, _resolve_all_inputs, prepare_inputs
from scripts.publish_infra_k8s_outputs import (
    RawOutputValues,
    publish_outputs,
    resolve_output_values,
)
from scripts.render_platform_manifests import (
    RawRenderInputs,
    build_render_tfvars,
    render_manifests,
    resolve_render_inputs,
)


def _apply_env_file(monkeypatch: pytest.MonkeyPatch, env_path: Path) -> None:
    lines = env_path.read_text(encoding="utf-8").splitlines()
    index = 0
    while index < len(lines):
        line = lines[index]
        if "<<" in line:
            key, marker = line.split("<<", 1)
            key = key.strip()
            marker = marker.strip()
            index += 1
            value_lines: list[str] = []
            while index < len(lines) and lines[index] != marker:
                value_lines.append(lines[index])
                index += 1
            monkeypatch.setenv(key, "\n".join(value_lines))
        elif "=" in line:
            key, value = line.split("=", 1)
            monkeypatch.setenv(key, value)
        index += 1


def _seed_environment(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> tuple[Path, Path]:
    env_file = tmp_path / "github-env"
    output_file = tmp_path / "github-output"
    runner_temp = tmp_path / "runner"
    runner_temp.mkdir(parents=True)

    env_vars = {
        "INPUT_CLUSTER_NAME": "preview-1",
        "INPUT_ENVIRONMENT": "preview",
        "INPUT_REGION": "nyc1",
        "INPUT_DOMAIN": "example.test",
        "INPUT_ACME_EMAIL": "admin@example.test",
        "INPUT_NODE_POOLS": "[]",
        "INPUT_GITOPS_REPOSITORY": "wildside/wildside-infra",
        "INPUT_GITOPS_TOKEN": "token",
        "INPUT_VAULT_ADDRESS": "https://vault.example.test:8200",
        "INPUT_VAULT_ROLE_ID": "role",
        "INPUT_VAULT_SECRET_ID": "secret",
        "INPUT_DIGITALOCEAN_TOKEN": "do-token",
        "INPUT_SPACES_ACCESS_KEY": "access",
        "INPUT_SPACES_SECRET_KEY": "secret",
        "INPUT_DRY_RUN": "true",
        "RUNNER_TEMP": str(runner_temp),
        "GITHUB_ENV": str(env_file),
        "GITHUB_OUTPUT": str(output_file),
        "RENDER_OUTPUT_DIR": str(tmp_path / "rendered"),
    }
    for key, value in env_vars.items():
        monkeypatch.setenv(key, value)

    return env_file, output_file


def _prepare_inputs(monkeypatch: pytest.MonkeyPatch, env_file: Path) -> None:
    raw_values = dict.fromkeys(RawInputs.__annotations__, None)
    raw_values["node_pools"] = "[]"
    inputs_resolved = _resolve_all_inputs(RawInputs(**raw_values))
    prepare_inputs(inputs_resolved)
    _apply_env_file(monkeypatch, env_file)


def _mock_provision(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(
        "scripts._provision_cluster_flow.tofu_init",
        lambda *_args, **_kwargs: TofuResult(
            success=True,
            stdout="",
            stderr="",
            return_code=0,
        ),
    )
    monkeypatch.setattr(
        "scripts._provision_cluster_flow.tofu_plan",
        lambda *_args, **_kwargs: TofuResult(
            success=True,
            stdout="",
            stderr="",
            return_code=0,
        ),
    )
    monkeypatch.setattr(
        "scripts._provision_cluster_flow.tofu_apply",
        lambda *_args, **_kwargs: TofuResult(
            success=True,
            stdout="",
            stderr="",
            return_code=0,
        ),
    )
    monkeypatch.setattr(
        "scripts._provision_cluster_flow.tofu_output",
        lambda *_args, **_kwargs: {"cluster_id": "abc", "endpoint": "https://kube"},
    )


def _run_provision_flow(monkeypatch: pytest.MonkeyPatch, env_file: Path) -> None:
    provision_inputs = resolve_provision_inputs(RawProvisionInputs())
    backend_config = build_backend_config(provision_inputs)
    cluster_tfvars = build_cluster_tfvars(provision_inputs)
    success, outputs = provision_cluster(provision_inputs, backend_config, cluster_tfvars)
    assert success is True
    export_cluster_outputs(provision_inputs, outputs)
    _apply_env_file(monkeypatch, env_file)


def _mock_render(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(
        "scripts.render_platform_manifests.run_tofu",
        lambda *_args, **_kwargs: TofuResult(
            success=True,
            stdout="",
            stderr="",
            return_code=0,
        ),
    )
    monkeypatch.setattr(
        "scripts.render_platform_manifests.tofu_output",
        lambda *_args, **_kwargs: {
            "rendered_manifests": {"value": {"platform/traefik.yaml": "apiVersion: v1"}}
        },
    )


def _run_render_flow(monkeypatch: pytest.MonkeyPatch) -> None:
    render_inputs = resolve_render_inputs(RawRenderInputs())
    render_tfvars = build_render_tfvars(render_inputs)
    manifests = render_manifests(render_inputs, render_tfvars)
    count = write_manifests(render_inputs.output_dir, manifests)
    append_github_env(
        render_inputs.github_env,
        {
            "RENDERED_MANIFEST_COUNT": str(count),
            "RENDER_OUTPUT_DIR": str(render_inputs.output_dir),
        },
    )
    _apply_env_file(monkeypatch, render_inputs.github_env)


def _run_gitops_flow(monkeypatch: pytest.MonkeyPatch, env_file: Path) -> None:
    gitops_inputs = resolve_gitops_inputs(RawGitOpsInputs())
    clone_dir = gitops_inputs.runner_temp / "gitops-clone"
    clone_dir.mkdir(parents=True, exist_ok=True)
    sync_manifests(gitops_inputs, clone_dir)
    append_github_env(gitops_inputs.github_env, {"GITOPS_COMMIT_SHA": "sha"})
    _apply_env_file(monkeypatch, env_file)


def _run_publish_flow(output_file: Path) -> None:
    values = resolve_output_values(RawOutputValues())
    publish_outputs(values, output_file)


def test_action_flow_happy_path(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    env_file, output_file = _seed_environment(monkeypatch, tmp_path)
    _prepare_inputs(monkeypatch, env_file)
    _mock_provision(monkeypatch)
    _run_provision_flow(monkeypatch, env_file)
    _mock_render(monkeypatch)
    _run_render_flow(monkeypatch)
    _run_gitops_flow(monkeypatch, env_file)
    _run_publish_flow(output_file)

    output_lines = output_file.read_text(encoding="utf-8")
    assert "cluster_name=preview-1" in output_lines
    assert "gitops_commit_sha=sha" in output_lines
    assert "rendered_manifest_count=1" in output_lines
