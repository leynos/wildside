"""Structural tests for the bootstrap-vault-appliance composite action."""

from __future__ import annotations

from pathlib import Path
from typing import Any

import yaml

ACTION_PATH = Path(__file__).resolve().parents[2] / \
    ".github/actions/bootstrap-vault-appliance/action.yml"


def _load_action() -> dict[str, Any]:
    return yaml.safe_load(ACTION_PATH.read_text(encoding="utf-8"))


def test_inputs_cover_credentials_and_env() -> None:
    action = _load_action()
    inputs = action["inputs"]

    assert inputs["environment"]["required"] is True
    assert inputs["vault_address"]["required"] is True

    digitalocean = inputs["digitalocean_token"]
    assert digitalocean["required"] is True
    assert digitalocean.get("secret") is True

    bootstrap_state = inputs["bootstrap_state"]
    assert bootstrap_state["required"] is False
    assert bootstrap_state.get("secret") is True


def test_bootstrap_step_invokes_helper_with_idempotent_flags() -> None:
    action = _load_action()
    steps = action["runs"]["steps"]
    bootstrap = next(
        step for step in steps if step["name"] == "Bootstrap Vault appliance"
    )
    run_script = bootstrap["run"]

    assert "uv run scripts/bootstrap_vault_appliance.py" in run_script
    assert "args+=" not in run_script


def test_publish_step_emits_expected_outputs() -> None:
    action = _load_action()
    outputs = action["outputs"]

    expected = {
        "vault-address",
        "ca-certificate-path",
        "state-file",
        "approle-role-id",
        "approle-secret-id",
    }
    assert expected.issubset(outputs.keys())

    publish_step = next(
        step for step in action["runs"]["steps"]
        if step["id"] == "publish"
    )
    publish_run = publish_step["run"]
    assert "uv run scripts/publish_bootstrap_outputs.py" in publish_run

    expected_wiring = {
        "vault-address": "${{ steps.publish.outputs.vault-address }}",
        "ca-certificate-path": "${{ steps.publish.outputs.ca-certificate-path }}",
        "state-file": "${{ steps.publish.outputs.state-file }}",
        "approle-role-id": "${{ steps.publish.outputs.approle-role-id }}",
        "approle-secret-id": "${{ steps.publish.outputs.approle-secret-id }}",
    }
    for key, value in expected_wiring.items():
        assert outputs[key]["value"] == value
