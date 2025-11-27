#!/usr/bin/env -S uv run python
# /// script
# requires-python = ">=3.13"
# dependencies = ["cyclopts>=2.9"]
# ///
"""Publish Vault bootstrap outputs in a GitHub Action-friendly format."""

from __future__ import annotations

import json
import os
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Callable

from cyclopts import App, Parameter

Mask = Callable[[str], None]

app = App(help="Emit bootstrap outputs and mask secrets for GitHub Actions.")


@dataclass(frozen=True)
class BootstrapOutputs:
    """Outputs exposed to downstream workflow steps."""

    vault_address: str
    state_file: Path
    approle_role_id: str
    approle_secret_id: str
    ca_certificate_path: Path | None


def _append_output(output_file: Path, lines: list[str]) -> None:
    output_file.parent.mkdir(parents=True, exist_ok=True)
    with output_file.open("a", encoding="utf-8") as handle:
        for line in lines:
            handle.write(f"{line}\n")


def publish_bootstrap_outputs(
    *,
    vault_address: str,
    state_file: Path,
    ca_certificate_path: Path | None,
    github_output: Path,
    mask: Mask,
) -> BootstrapOutputs:
    """Read the state file, mask secrets, and export outputs."""

    if not state_file.exists():
        msg = f"State file not found: {state_file}"
        raise FileNotFoundError(msg)

    state = json.loads(state_file.read_text(encoding="utf-8"))

    approle_role_id = state.get("approle_role_id") or ""
    approle_secret_id = state.get("approle_secret_id") or ""
    root_token = state.get("root_token") or ""
    unseal_keys = state.get("unseal_keys") or []

    for secret in [approle_secret_id, root_token, *unseal_keys]:
        if secret:
            mask(f"::add-mask::{secret}")

    output_lines = [
        f"vault-address={vault_address}",
        f"state-file={state_file}",
        f"approle-role-id={approle_role_id}",
        f"approle-secret-id={approle_secret_id}",
        f"ca-certificate-path={ca_certificate_path or ''}",
    ]
    _append_output(github_output, output_lines)

    return BootstrapOutputs(
        vault_address=vault_address,
        state_file=state_file,
        approle_role_id=approle_role_id,
        approle_secret_id=approle_secret_id,
        ca_certificate_path=ca_certificate_path,
    )


@app.command()
def main(
    vault_address: str | None = Parameter(),
    state_file: Path | None = Parameter(),
    ca_certificate_path: Path | None = Parameter(),
    github_output: Path | None = Parameter(),
) -> None:
    """CLI entrypoint used by the composite action."""

    vault_address = vault_address or os.environ.get("VAULT_ADDRESS")
    if vault_address is None:
        raise SystemExit("VAULT_ADDRESS is required")

    state_file = state_file or Path(os.environ["STATE_FILE"])
    github_output = github_output or Path(os.environ["GITHUB_OUTPUT"])
    if "CA_CERT_PATH" in os.environ and ca_certificate_path is None:
        ca_certificate_path = Path(os.environ["CA_CERT_PATH"])

    publish_bootstrap_outputs(
        vault_address=vault_address,
        state_file=state_file,
        ca_certificate_path=ca_certificate_path,
        github_output=github_output,
        mask=print,
    )


if __name__ == "__main__":  # pragma: no cover - exercised via CLI
    try:
        app()
    except Exception as exc:  # noqa: BLE001 - propagate friendly message
        msg = f"bootstrap output publication failed: {exc}"
        print(msg, file=sys.stderr)
        raise
