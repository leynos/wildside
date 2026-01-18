"""Unit tests for render_platform_manifests."""

from __future__ import annotations

from pathlib import Path

import pytest

from scripts._infra_k8s import TofuResult
from scripts.render_platform_manifests import (
    RawRenderInputs,
    RenderInputs,
    _extract_rendered_manifests,
    build_render_tfvars,
    render_manifests,
    resolve_render_inputs,
)


def test_resolve_render_inputs_env(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    monkeypatch.setenv("CLUSTER_NAME", "preview")
    monkeypatch.setenv("DOMAIN", "example.test")
    monkeypatch.setenv("ACME_EMAIL", "admin@example.test")
    monkeypatch.setenv("ENABLE_TRAEFIK", "false")
    monkeypatch.setenv("RUNNER_TEMP", str(tmp_path))
    monkeypatch.setenv("RENDER_OUTPUT_DIR", str(tmp_path / "render"))
    monkeypatch.setenv("GITHUB_ENV", str(tmp_path / "env"))

    inputs = resolve_render_inputs(RawRenderInputs())
    assert inputs.cluster_name == "preview"
    assert inputs.enable_traefik is False


def test_resolve_render_inputs_cli_override(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("CLUSTER_NAME", "env")
    monkeypatch.setenv("DOMAIN", "example.test")
    monkeypatch.setenv("ACME_EMAIL", "admin@example.test")

    inputs = resolve_render_inputs(RawRenderInputs(cluster_name="cli"))
    assert inputs.cluster_name == "cli"


def test_build_render_tfvars_skips_vault_when_disabled(tmp_path: Path) -> None:
    inputs = RenderInputs(
        cluster_name="preview",
        domain="example.test",
        acme_email="admin@example.test",
        cloudflare_api_token_secret_name="cloudflare-api-token",
        vault_address="https://vault.example",
        vault_role_id="role",
        vault_secret_id="secret",
        vault_ca_certificate="cert",
        enable_traefik=True,
        enable_cert_manager=True,
        enable_external_dns=True,
        enable_vault_eso=False,
        enable_cnpg=True,
        runner_temp=tmp_path,
        output_dir=tmp_path / "out",
        github_env=tmp_path / "env",
    )

    tfvars = build_render_tfvars(inputs)
    assert "vault_address" not in tfvars
    assert tfvars["vault_eso_enabled"] is False


def test_extract_rendered_manifests_handles_wrapped_value() -> None:
    outputs = {
        "rendered_manifests": {
            "value": {"platform/traefik.yaml": "apiVersion: v1"}
        }
    }
    manifests = _extract_rendered_manifests(outputs)
    assert manifests == {"platform/traefik.yaml": "apiVersion: v1"}


def test_render_manifests_runs_tofu(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    inputs = RenderInputs(
        cluster_name="preview",
        domain="example.test",
        acme_email="admin@example.test",
        cloudflare_api_token_secret_name="cloudflare-api-token",
        vault_address=None,
        vault_role_id=None,
        vault_secret_id=None,
        vault_ca_certificate=None,
        enable_traefik=True,
        enable_cert_manager=True,
        enable_external_dns=True,
        enable_vault_eso=True,
        enable_cnpg=True,
        runner_temp=tmp_path,
        output_dir=tmp_path / "out",
        github_env=tmp_path / "env",
    )
    tfvars = build_render_tfvars(inputs)

    calls: list[list[str]] = []

    def fake_run_tofu(args: list[str], _cwd: Path) -> TofuResult:
        calls.append(args)
        return TofuResult(success=True, stdout="", stderr="", return_code=0)

    monkeypatch.setattr("scripts.render_platform_manifests.run_tofu", fake_run_tofu)
    monkeypatch.setattr(
        "scripts.render_platform_manifests.tofu_output",
        lambda *_args, **_kwargs: {
            "rendered_manifests": {"value": {"platform/traefik.yaml": "apiVersion: v1"}}
        },
    )

    manifests = render_manifests(inputs, tfvars)
    assert "platform/traefik.yaml" in manifests
    assert len(calls) == 2
