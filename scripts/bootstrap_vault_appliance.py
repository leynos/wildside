#!/usr/bin/env -S uv run python
# /// script
# requires-python = ">=3.13"
# dependencies = ["plumbum"]
# ///

"""Vault appliance bootstrap helper."""

from __future__ import annotations

import os
import sys
from collections import abc as cabc
from dataclasses import dataclass
from pathlib import Path

from cyclopts import App, Parameter

REPO_ROOT = Path(__file__).resolve().parents[1]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from scripts._vault_bootstrap import (
    VaultBootstrapConfig,
    VaultBootstrapError,
    bootstrap,
)

app = App(help="Bootstrap the Vault appliance via environment-derived inputs.")


@dataclass(frozen=True, slots=True)
class InputResolution:
    """Configuration for resolving an input from multiple sources."""

    env_key: str
    default: str | Path | None = None
    required: bool = False
    as_path: bool = False


def _resolve_input(
    param_value: str | Path | None,
    resolution: InputResolution,
    env: cabc.Mapping[str, str] | None = None,
) -> str | Path | None:
    """Resolve an input from CLI parameter or environment variables."""

    if param_value is not None:
        return param_value

    env_value = (env or os.environ).get(resolution.env_key)
    if env_value is not None:
        return Path(env_value) if resolution.as_path else env_value

    if resolution.required:
        raise SystemExit(f"{resolution.env_key} is required")

    return resolution.default


def _ensure_policy_path(
    approle_policy_path: Path | None,
    approle_policy_content: str | None,
    state_file: Path,
) -> Path | None:
    """Persist inline policy content alongside the state file when provided."""

    if approle_policy_path is not None:
        return approle_policy_path
    if not approle_policy_content:
        return None
    destination = state_file.parent / "approle-policy.hcl"
    destination.write_text(approle_policy_content, encoding="utf-8")
    destination.chmod(0o600)
    return destination


def _to_bool(value: str | bool | None) -> bool:
    if isinstance(value, bool):
        return value
    if value is None:
        return False
    return value.strip().lower() in {"1", "true", "yes", "on"}


def build_config(
    *,
    vault_addr: str | None,
    droplet_tag: str | None,
    state_file: Path | None,
    ssh_user: str | None,
    ssh_identity: Path | None,
    ca_certificate: Path | None,
    kv_mount_path: str | None,
    approle_name: str | None,
    approle_policy_name: str | None,
    approle_policy_path: Path | None,
    approle_policy_content: str | None,
    key_shares: int | None,
    key_threshold: int | None,
    token_ttl: str | None,
    token_max_ttl: str | None,
    secret_id_ttl: str | None,
    rotate_secret_id: bool | str | None,
    env: cabc.Mapping[str, str] | None = None,
) -> VaultBootstrapConfig:
    """Build a bootstrap configuration from CLI parameters and environment."""

    env = env or os.environ
    resolved_state_file = _resolve_input(
        state_file,
        InputResolution(env_key="STATE_FILE", required=True, as_path=True),
        env=env,
    )
    resolved_vault_addr = _resolve_input(
        vault_addr,
        InputResolution(env_key="VAULT_ADDRESS", required=True),
        env=env,
    )
    resolved_droplet_tag = _resolve_input(
        droplet_tag,
        InputResolution(env_key="DROPLET_TAG", required=True),
        env=env,
    )
    resolved_ssh_user = _resolve_input(
        ssh_user,
        InputResolution(env_key="SSH_USER", default="root"),
        env=env,
    )
    resolved_ssh_identity = _resolve_input(
        ssh_identity,
        InputResolution(env_key="SSH_IDENTITY", as_path=True),
        env=env,
    )
    resolved_ca_certificate = _resolve_input(
        ca_certificate,
        InputResolution(env_key="CA_CERT_PATH", as_path=True),
        env=env,
    )
    resolved_kv_mount_path = _resolve_input(
        kv_mount_path,
        InputResolution(env_key="KV_MOUNT_PATH", default="secret"),
        env=env,
    )
    resolved_approle_name = _resolve_input(
        approle_name,
        InputResolution(env_key="APPROLE_NAME", default="doks-deployer"),
        env=env,
    )
    resolved_approle_policy_name = _resolve_input(
        approle_policy_name,
        InputResolution(env_key="APPROLE_POLICY_NAME", default="doks-deployer"),
        env=env,
    )
    resolved_policy_content = _resolve_input(
        approle_policy_content,
        InputResolution(env_key="APPROLE_POLICY", default=None),
        env=env,
    )
    resolved_approle_policy_path = _resolve_input(
        approle_policy_path,
        InputResolution(env_key="APPROLE_POLICY_PATH", as_path=True),
        env=env,
    )

    resolved_key_shares = int(
        _resolve_input(
            key_shares,
            InputResolution(env_key="KEY_SHARES", default="5"),
            env=env,
        )
    )
    resolved_key_threshold = int(
        _resolve_input(
            key_threshold,
            InputResolution(env_key="KEY_THRESHOLD", default="3"),
            env=env,
        )
    )
    if resolved_key_threshold > resolved_key_shares:
        raise SystemExit("--key-threshold must be ≤ --key-shares")

    resolved_token_ttl = _resolve_input(
        token_ttl,
        InputResolution(env_key="TOKEN_TTL", default="1h"),
        env=env,
    )
    resolved_token_max_ttl = _resolve_input(
        token_max_ttl,
        InputResolution(env_key="TOKEN_MAX_TTL", default="4h"),
        env=env,
    )
    resolved_secret_id_ttl = _resolve_input(
        secret_id_ttl,
        InputResolution(env_key="SECRET_ID_TTL", default="4h"),
        env=env,
    )
    resolved_rotate_secret = _to_bool(
        _resolve_input(
            rotate_secret_id,
            InputResolution(env_key="ROTATE_SECRET_ID", default="false"),
            env=env,
        )
    )

    policy_path = _ensure_policy_path(
        resolved_approle_policy_path if isinstance(resolved_approle_policy_path, Path) else None,
        str(resolved_policy_content) if resolved_policy_content is not None else None,
        resolved_state_file,
    )

    return VaultBootstrapConfig(
        vault_addr=str(resolved_vault_addr),
        droplet_tag=str(resolved_droplet_tag),
        state_file=resolved_state_file,
        ssh_user=str(resolved_ssh_user),
        ssh_identity=resolved_ssh_identity if isinstance(resolved_ssh_identity, Path) else None,
        kv_mount_path=str(resolved_kv_mount_path),
        approle_name=str(resolved_approle_name),
        approle_policy_name=str(resolved_approle_policy_name),
        approle_policy_path=policy_path,
        key_shares=resolved_key_shares,
        key_threshold=resolved_key_threshold,
        token_ttl=str(resolved_token_ttl),
        token_max_ttl=str(resolved_token_max_ttl),
        secret_id_ttl=str(resolved_secret_id_ttl),
        rotate_secret_id=resolved_rotate_secret,
        ca_certificate=resolved_ca_certificate if isinstance(resolved_ca_certificate, Path) else None,
    )


@app.command()
def main(
    vault_addr: str | None = Parameter(),
    droplet_tag: str | None = Parameter(),
    state_file: Path | None = Parameter(),
    ssh_user: str | None = Parameter(),
    ssh_identity: Path | None = Parameter(),
    ca_certificate: Path | None = Parameter(),
    kv_mount_path: str | None = Parameter(),
    approle_name: str | None = Parameter(),
    approle_policy_name: str | None = Parameter(),
    approle_policy_path: Path | None = Parameter(),
    approle_policy_content: str | None = Parameter(),
    key_shares: int | None = Parameter(),
    key_threshold: int | None = Parameter(),
    token_ttl: str | None = Parameter(),
    token_max_ttl: str | None = Parameter(),
    secret_id_ttl: str | None = Parameter(),
    rotate_secret_id: bool | str | None = Parameter(),
) -> int:
    """Entry point for command-line execution."""

    config = build_config(
        vault_addr=vault_addr,
        droplet_tag=droplet_tag,
        state_file=state_file,
        ssh_user=ssh_user,
        ssh_identity=ssh_identity,
        ca_certificate=ca_certificate,
        kv_mount_path=kv_mount_path,
        approle_name=approle_name,
        approle_policy_name=approle_policy_name,
        approle_policy_path=approle_policy_path,
        approle_policy_content=approle_policy_content,
        key_shares=key_shares,
        key_threshold=key_threshold,
        token_ttl=token_ttl,
        token_max_ttl=token_max_ttl,
        secret_id_ttl=secret_id_ttl,
        rotate_secret_id=rotate_secret_id,
    )
    try:
        state = bootstrap(config)
    except VaultBootstrapError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1
    print("Vault appliance bootstrap complete.")
    if state.approle_role_id and state.approle_secret_id:
        print("AppRole credentials available in the state file for downstream use.")
    return 0


if __name__ == "__main__":  # pragma: no cover - CLI entrypoint
    raise SystemExit(app())
