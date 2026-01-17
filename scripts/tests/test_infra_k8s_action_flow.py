"""Behavioural test for wildside-infra-k8s action flow."""

from __future__ import annotations

import sys
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from scripts._infra_k8s import (  # noqa: E402
    TofuResult,
    append_github_env,
    write_manifests,
)
from scripts.commit_gitops_manifests import (  # noqa: E402
    resolve_gitops_inputs,
    sync_manifests,
)
from scripts.prepare_infra_k8s_inputs import (  # noqa: E402
    RawInputs,
    _resolve_all_inputs,
    prepare_inputs,
)
from scripts.provision_cluster import (  # noqa: E402
    build_backend_config,
    build_tfvars as build_cluster_tfvars,
    export_cluster_outputs,
    provision_cluster,
    resolve_provision_inputs,
)
from scripts.publish_infra_k8s_outputs import (  # noqa: E402
    publish_outputs,
    resolve_output_values,
)
from scripts.render_platform_manifests import (  # noqa: E402
    build_render_tfvars,
    render_manifests,
    resolve_render_inputs,
)


def _apply_env_file(monkeypatch: pytest.MonkeyPatch, env_path: Path) -> None:
    for line in env_path.read_text(encoding="utf-8").splitlines():
        if "<<" in line:
            continue
        if "=" not in line:
            continue
        key, value = line.split("=", 1)
        monkeypatch.setenv(key, value)


def test_action_flow_happy_path(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    env_file = tmp_path / "github-env"
    output_file = tmp_path / "github-output"
    render_dir = tmp_path / "rendered"
    runner_temp = tmp_path / "runner"
    runner_temp.mkdir(parents=True)

    inputs = {
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
        "RENDER_OUTPUT_DIR": str(render_dir),
    }
    for key, value in inputs.items():
        monkeypatch.setenv(key, value)

    raw_values = {field: None for field in RawInputs.__annotations__}
    raw_values["node_pools"] = "[]"
    inputs_resolved = _resolve_all_inputs(RawInputs(**raw_values))
    prepare_inputs(inputs_resolved)
    _apply_env_file(monkeypatch, env_file)

    monkeypatch.setattr(
        "scripts.provision_cluster.tofu_init",
        lambda *_args, **_kwargs: TofuResult(True, "", "", 0),
    )
    monkeypatch.setattr(
        "scripts.provision_cluster.tofu_plan",
        lambda *_args, **_kwargs: TofuResult(True, "", "", 0),
    )
    monkeypatch.setattr(
        "scripts.provision_cluster.tofu_apply",
        lambda *_args, **_kwargs: TofuResult(True, "", "", 0),
    )
    monkeypatch.setattr(
        "scripts.provision_cluster.tofu_output",
        lambda *_args, **_kwargs: {"cluster_id": "abc", "endpoint": "https://kube"},
    )

    provision_inputs = resolve_provision_inputs()
    backend_config = build_backend_config(provision_inputs)
    cluster_tfvars = build_cluster_tfvars(provision_inputs)
    success, outputs = provision_cluster(provision_inputs, backend_config, cluster_tfvars)
    assert success is True
    export_cluster_outputs(provision_inputs, outputs)
    _apply_env_file(monkeypatch, env_file)

    monkeypatch.setattr(
        "scripts.render_platform_manifests.run_tofu",
        lambda *_args, **_kwargs: TofuResult(True, "", "", 0),
    )
    monkeypatch.setattr(
        "scripts.render_platform_manifests.tofu_output",
        lambda *_args, **_kwargs: {
            "rendered_manifests": {"value": {"platform/traefik.yaml": "apiVersion: v1"}}
        },
    )

    render_inputs = resolve_render_inputs()
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
    _apply_env_file(monkeypatch, env_file)

    gitops_inputs = resolve_gitops_inputs()
    clone_dir = gitops_inputs.runner_temp / "gitops-clone"
    clone_dir.mkdir(parents=True, exist_ok=True)
    sync_manifests(gitops_inputs, clone_dir)
    append_github_env(gitops_inputs.github_env, {"GITOPS_COMMIT_SHA": "sha"})
    _apply_env_file(monkeypatch, env_file)

    values = resolve_output_values(
        cluster_name=None,
        cluster_id=None,
        cluster_endpoint=None,
        gitops_commit_sha=None,
        rendered_manifest_count=None,
    )
    publish_outputs(values, output_file)

    output_lines = output_file.read_text(encoding="utf-8")
    assert "cluster_name=preview-1" in output_lines
    assert "gitops_commit_sha=sha" in output_lines
    assert "rendered_manifest_count=1" in output_lines
