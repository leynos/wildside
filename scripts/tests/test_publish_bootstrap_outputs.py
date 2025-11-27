"""Unit tests for the publish_bootstrap_outputs helper."""

from __future__ import annotations

from pathlib import Path

import pytest

from scripts.publish_bootstrap_outputs import (
    BootstrapOutputs,
    publish_bootstrap_outputs,
)


def test_masks_secrets_and_writes_outputs(tmp_path: Path) -> None:
    state_file = tmp_path / "state.json"
    state_file.write_text(
        """
        {
          "approle_role_id": "role-123",
          "approle_secret_id": "secret-abc",
          "root_token": "root-xyz",
          "unseal_keys": ["key1", "key2"]
        }
        """,
        encoding="utf-8",
    )
    output_file = tmp_path / "out"
    masks: list[str] = []

    result = publish_bootstrap_outputs(
        vault_address="https://vault",
        state_file=state_file,
        ca_certificate_path=tmp_path / "ca.pem",
        github_output=output_file,
        mask=masks.append,
    )

    assert isinstance(result, BootstrapOutputs)
    assert result.approle_role_id == "role-123"
    assert result.approle_secret_id == "secret-abc"

    lines = output_file.read_text(encoding="utf-8").splitlines()
    assert "vault-address=https://vault" in lines
    assert f"state-file={state_file}" in lines
    assert "approle-role-id=role-123" in lines
    assert "approle-secret-id=secret-abc" in lines
    assert f"ca-certificate-path={tmp_path / 'ca.pem'}" in lines

    assert masks == [
        "::add-mask::secret-abc",
        "::add-mask::root-xyz",
        "::add-mask::key1",
        "::add-mask::key2",
    ]


def test_missing_state_file_errors(tmp_path: Path) -> None:
    output_file = tmp_path / "out"
    with pytest.raises(FileNotFoundError):
        publish_bootstrap_outputs(
            vault_address="https://vault",
            state_file=tmp_path / "missing.json",
            ca_certificate_path=None,
            github_output=output_file,
            mask=lambda _secret: None,
        )
