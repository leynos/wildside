#!/usr/bin/env -S uv run python
# /// script
# requires-python = ">=3.13"
# dependencies = ["cyclopts>=2.9"]
# ///
"""Prepare Vault bootstrap inputs for the composite GitHub Action.

The script replaces the inline shell+Python snippets in the composite action
with a small, testable helper. It:

- resolves default paths for the state file, CA certificate, and SSH identity;
- materialises any provided bootstrap payloads (raw or base64-encoded);
- writes GitHub Action environment exports to ``$GITHUB_ENV``; and
- masks sensitive SSH keys in logs.
"""

from __future__ import annotations

import base64
import json
import os
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Callable

from cyclopts import App, Parameter

Mask = Callable[[str], None]

app = App(help="Materialise Vault bootstrap inputs and export env values.")


@dataclass(frozen=True)
class GitHubActionContext:
    """Contextual paths and helpers provided by the GitHub Action runner."""

    runner_temp: Path
    github_env: Path
    github_output: Path | None = None
    mask: Mask = print


@dataclass(frozen=True)
class BootstrapPayloads:
    """Optional payloads supplied to seed bootstrap artefacts."""

    bootstrap_state: str | None = None
    ca_certificate: str | None = None
    ssh_key: str | None = None


@dataclass(frozen=True)
class VaultEnvironmentConfig:
    """Configuration describing the Vault environment under bootstrap."""

    environment: str
    droplet_tag: str | None = None
    state_path: Path | None = None
    vault_address: str | None = None


@dataclass(frozen=True)
class PreparedPaths:
    """Resolved paths produced by the preparation step."""

    droplet_tag: str
    state_file: Path
    ca_certificate_path: Path | None
    ssh_identity_path: Path | None


def _decode_payload(payload: str) -> str:
    """Return payload as text, decoding base64 if it is valid."""

    candidate = payload.strip()
    if not candidate:
        return ""

    try:
        decoded = base64.b64decode(candidate, validate=True)
    except (base64.binascii.Error, ValueError):
        return candidate

    try:
        return decoded.decode("utf-8")
    except UnicodeDecodeError:
        return candidate


def _write_payload_file(payload: str, destination: Path) -> None:
    """Write decoded payload to destination, validating JSON when applicable."""

    decoded = _decode_payload(payload)
    if not decoded:
        return

    destination.parent.mkdir(parents=True, exist_ok=True)

    if destination.suffix == ".json":
        try:
            parsed = json.loads(decoded)
        except json.JSONDecodeError as exc:  # pragma: no cover - safety net
            msg = f"Invalid JSON supplied for {destination.name}: {exc}"
            raise SystemExit(msg) from exc
        decoded = json.dumps(parsed, indent=2)

    destination.write_text(decoded, encoding="utf-8")
    destination.chmod(0o600)


def _append_env(env_file: Path, lines: list[str]) -> None:
    env_file.parent.mkdir(parents=True, exist_ok=True)
    with env_file.open("a", encoding="utf-8") as handle:
        for line in lines:
            handle.write(f"{line}\n")


def _handle_state_file(
    state_path: Path | None,
    environment: str,
    runner_temp: Path,
    bootstrap_state: str | None,
) -> Path:
    """Resolve and materialise the bootstrap state file when provided."""

    state_file = state_path or runner_temp / "vault-bootstrap" / environment / "state.json"
    if not state_file.exists() and bootstrap_state:
        _write_payload_file(bootstrap_state, state_file)
    return state_file


def _handle_ca_certificate(
    ca_certificate: str | None,
    state_file: Path,
) -> Path | None:
    """Materialise the CA certificate payload when present."""

    if not ca_certificate:
        return None
    ca_cert_path = state_file.parent / "vault-ca.pem"
    _write_payload_file(ca_certificate, ca_cert_path)
    return ca_cert_path


def _handle_ssh_identity(
    ssh_key: str | None,
    state_file: Path,
    mask: Mask,
) -> Path | None:
    """Materialise the SSH identity file and mask its contents."""

    if not ssh_key:
        return None
    ssh_identity = state_file.parent / "vault-ssh-key"
    ssh_identity.parent.mkdir(parents=True, exist_ok=True)
    ssh_identity.write_text(f"{ssh_key}\n", encoding="utf-8")
    ssh_identity.chmod(0o600)
    mask(f"::add-mask::{ssh_key}")
    return ssh_identity


def prepare_bootstrap_inputs(
    *,
    vault_config: VaultEnvironmentConfig,
    payloads: BootstrapPayloads,
    github_context: GitHubActionContext,
) -> PreparedPaths:
    """Materialise input payloads and export paths to GITHUB_ENV."""

    resolved_droplet_tag = vault_config.droplet_tag or f"vault-{vault_config.environment}"
    state_file = _handle_state_file(
        vault_config.state_path,
        vault_config.environment,
        github_context.runner_temp,
        payloads.bootstrap_state,
    )
    ca_cert_path = _handle_ca_certificate(payloads.ca_certificate, state_file)
    ssh_identity = _handle_ssh_identity(
        payloads.ssh_key,
        state_file,
        github_context.mask,
    )

    env_lines = [
        f"DROPLET_TAG={resolved_droplet_tag}",
        f"STATE_FILE={state_file}",
    ]
    if ca_cert_path:
        env_lines.append(f"CA_CERT_PATH={ca_cert_path}")
    if ssh_identity:
        env_lines.append(f"SSH_IDENTITY={ssh_identity}")

    _append_env(github_context.github_env, env_lines)

    return PreparedPaths(
        droplet_tag=resolved_droplet_tag,
        state_file=state_file,
        ca_certificate_path=ca_cert_path,
        ssh_identity_path=ssh_identity,
    )


@app.command()
def main(
    environment: str | None = Parameter(),
    runner_temp: Path | None = Parameter(),
    droplet_tag: str | None = Parameter(),
    state_path: Path | None = Parameter(),
    bootstrap_state: str | None = Parameter(),
    ca_certificate: str | None = Parameter(),
    ssh_key: str | None = Parameter(),
    github_env: Path | None = Parameter(),
) -> None:
    """CLI entrypoint used by the composite action."""

    environment = environment or os.environ.get("INPUT_ENVIRONMENT")
    if environment is None:
        raise SystemExit("INPUT_ENVIRONMENT is required")

    runner_temp = runner_temp or Path(os.environ.get("RUNNER_TEMP", "/tmp"))
    github_env = github_env or Path(
        os.environ.get("GITHUB_ENV", "/tmp/github-env-undefined")
    )
    droplet_tag = droplet_tag or os.environ.get("INPUT_DROPLET_TAG")
    state_path = state_path or (
        Path(os.environ["INPUT_STATE_PATH"])
        if "INPUT_STATE_PATH" in os.environ
        else None
    )
    bootstrap_state = bootstrap_state or os.environ.get("INPUT_BOOTSTRAP_STATE")
    ca_certificate = ca_certificate or os.environ.get("INPUT_CA_CERTIFICATE")
    ssh_key = ssh_key or os.environ.get("INPUT_SSH_KEY")

    vault_config = VaultEnvironmentConfig(
        environment=environment,
        droplet_tag=droplet_tag,
        state_path=state_path,
        vault_address=None,
    )
    payloads = BootstrapPayloads(
        bootstrap_state=bootstrap_state,
        ca_certificate=ca_certificate,
        ssh_key=ssh_key,
    )
    github_context = GitHubActionContext(
        runner_temp=runner_temp,
        github_env=github_env,
        github_output=None,
        mask=print,
    )

    prepare_bootstrap_inputs(
        vault_config=vault_config,
        payloads=payloads,
        github_context=github_context,
    )


if __name__ == "__main__":  # pragma: no cover - exercised via CLI
    try:
        app()
    except Exception as exc:  # noqa: BLE001 - propagate friendly message
        msg = f"bootstrap input preparation failed: {exc}"
        print(msg, file=sys.stderr)
        raise
