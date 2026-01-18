"""Unit tests for prepare_infra_k8s_inputs."""

from __future__ import annotations

import json
import secrets
from pathlib import Path

import pytest

from scripts.prepare_infra_k8s_inputs import (
    RawInputs,
    _resolve_all_inputs,
    prepare_inputs,
)


def _base_env(tmp_path: Path) -> dict[str, str]:
    token = _dummy_token()
    do_token = _dummy_token()
    return {
        "INPUT_CLUSTER_NAME": "Preview-1",
        "INPUT_ENVIRONMENT": "preview",
        "INPUT_REGION": "nyc1",
        "INPUT_DOMAIN": "example.test",
        "INPUT_ACME_EMAIL": "admin@example.test",
        "INPUT_GITOPS_REPOSITORY": "wildside/wildside-infra",
        "INPUT_GITOPS_TOKEN": token,
        "INPUT_VAULT_ADDRESS": "https://vault.example.test:8200",
        "INPUT_VAULT_ROLE_ID": "role-id",
        "INPUT_VAULT_SECRET_ID": "secret-id",
        "INPUT_DIGITALOCEAN_TOKEN": do_token,
        "INPUT_SPACES_ACCESS_KEY": "spaces-key",
        "INPUT_SPACES_SECRET_KEY": "spaces-secret",
        "RUNNER_TEMP": str(tmp_path / "runner"),
        "GITHUB_ENV": str(tmp_path / "env"),
    }


def _dummy_token() -> str:
    return f"token-{secrets.token_hex(8)}"


def test_resolve_all_inputs_happy_path(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    env = _base_env(tmp_path)
    env.update(
        {
            "INPUT_NODE_POOLS": json.dumps(
                [
                    {
                        "name": "default",
                        "size": "s-2vcpu-2gb",
                        "node_count": 2,
                        "auto_scale": False,
                        "min_nodes": 2,
                        "max_nodes": 2,
                    }
                ]
            ),
            "INPUT_ENABLE_TRAEFIK": "false",
            "INPUT_DRY_RUN": "true",
        }
    )
    for key, value in env.items():
        monkeypatch.setenv(key, value)

    raw_values = dict.fromkeys(RawInputs.__annotations__, None)
    inputs = _resolve_all_inputs(RawInputs(**raw_values))

    assert inputs.cluster_name == "preview-1", "Cluster name should normalize"
    assert inputs.enable_traefik is False, "Traefik flag should parse to False"
    assert inputs.dry_run is True, "Dry run flag should parse to True"
    assert inputs.node_pools is not None, "Node pools should be parsed"
    assert inputs.node_pools[0]["name"] == "default", "Expected default pool name"


def test_resolve_all_inputs_rejects_invalid_cluster(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    env = _base_env(tmp_path)
    env["INPUT_CLUSTER_NAME"] = "Invalid_Name"
    for key, value in env.items():
        monkeypatch.setenv(key, value)

    raw_values = dict.fromkeys(RawInputs.__annotations__, None)
    with pytest.raises(ValueError, match="cluster_name"):
        _resolve_all_inputs(RawInputs(**raw_values))


def test_prepare_inputs_masks_and_exports(tmp_path: Path) -> None:
    env_file = tmp_path / "env"
    masks: list[str] = []
    gitops_token = _dummy_token()
    do_token = _dummy_token()

    inputs = _resolve_all_inputs(
        RawInputs(
            cluster_name="preview-2",
            environment="preview",
            region="nyc1",
            kubernetes_version=None,
            node_pools=None,
            domain="example.test",
            acme_email="admin@example.test",
            gitops_repository="wildside/wildside-infra",
            gitops_branch="main",
            gitops_token=gitops_token,
            vault_address="https://vault.example.test:8200",
            vault_role_id="role-id",
            vault_secret_id="secret-id",
            vault_ca_certificate="CERT\nLINE",
            digitalocean_token=do_token,
            spaces_access_key="spaces-key",
            spaces_secret_key="spaces-secret",
            cloudflare_api_token_secret_name="cloudflare-api-token",
            enable_traefik="true",
            enable_cert_manager="true",
            enable_external_dns="true",
            enable_vault_eso="true",
            enable_cnpg="true",
            dry_run="false",
            runner_temp=tmp_path,
            github_env=env_file,
        )
    )

    prepare_inputs(inputs, mask=masks.append)

    env_content = env_file.read_text(encoding="utf-8")
    assert "CLUSTER_NAME=preview-2" in env_content, "Cluster name should export"
    assert "VAULT_CA_CERTIFICATE<<" in env_content, "Vault CA should export multiline"

    assert "CERT\nLINE" in masks, "Vault CA should be masked"
    assert gitops_token in masks, "GitOps token should be masked"
    assert do_token in masks, "DigitalOcean token should be masked"
