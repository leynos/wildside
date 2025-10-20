#!/usr/bin/env -S uv run python
# /// script
# requires-python = ">=3.13"
# dependencies = ["plumbum"]
# ///

"""Vault appliance bootstrap helper."""

from __future__ import annotations

import argparse
from pathlib import Path

from _vault_bootstrap import (
    VaultBootstrapConfig,
    VaultBootstrapError,
    bootstrap,
)


def _add_infrastructure_args(parser: argparse.ArgumentParser) -> None:
    """Add infrastructure and connection arguments to the parser."""

    parser.add_argument("--vault-addr", required=True, help="Vault HTTPS endpoint")
    parser.add_argument("--droplet-tag", required=True, help="DigitalOcean tag")
    parser.add_argument(
        "--state-file",
        required=True,
        type=Path,
        help="Path to the bootstrap state file",
    )
    parser.add_argument(
        "--ssh-user",
        default="root",
        help="SSH user for droplet health checks (default: root)",
    )
    parser.add_argument(
        "--ssh-identity",
        type=Path,
        help="Optional SSH identity file passed via -i",
    )
    parser.add_argument(
        "--ca-certificate",
        type=Path,
        help="Path to the Vault CA certificate for TLS verification",
    )


def _add_vault_init_args(parser: argparse.ArgumentParser) -> None:
    """Add Vault initialisation arguments to the parser."""

    parser.add_argument(
        "--key-shares",
        type=int,
        default=5,
        help="Number of unseal key shares to generate",
    )
    parser.add_argument(
        "--key-threshold",
        type=int,
        default=3,
        help="Number of shares required to unseal Vault",
    )


def _add_kv_args(parser: argparse.ArgumentParser) -> None:
    """Add KV secrets engine arguments to the parser."""

    parser.add_argument(
        "--kv-mount-path",
        default="secret",
        help="Mount path for the KV v2 secrets engine",
    )


def _add_approle_args(parser: argparse.ArgumentParser) -> None:
    """Add AppRole configuration arguments to the parser."""

    parser.add_argument(
        "--approle-name",
        default="doks-deployer",
        help="Name of the AppRole used by the DOKS workflow",
    )
    parser.add_argument(
        "--approle-policy-name",
        default="doks-deployer",
        help="Name of the Vault policy attached to the AppRole",
    )
    parser.add_argument(
        "--approle-policy-path",
        type=Path,
        help="Path to a policy file overriding the default capabilities",
    )
    parser.add_argument(
        "--token-ttl",
        default="1h",
        help="TTL applied to tokens issued via the AppRole",
    )
    parser.add_argument(
        "--token-max-ttl",
        default="4h",
        help="Maximum TTL for AppRole tokens",
    )
    parser.add_argument(
        "--secret-id-ttl",
        default="4h",
        help="TTL for generated secret IDs",
    )
    parser.add_argument(
        "--rotate-secret-id",
        action="store_true",
        help="Rotate the AppRole secret ID even if one is already recorded",
    )


def parse_args(argv: list[str] | None = None) -> VaultBootstrapConfig:
    """Parse command-line arguments into a configuration object."""

    parser = argparse.ArgumentParser(description=__doc__)
    _add_infrastructure_args(parser)
    _add_vault_init_args(parser)
    _add_kv_args(parser)
    _add_approle_args(parser)
    args = parser.parse_args(argv)
    return VaultBootstrapConfig(
        vault_addr=args.vault_addr,
        droplet_tag=args.droplet_tag,
        state_file=args.state_file,
        ssh_user=args.ssh_user,
        ssh_identity=args.ssh_identity,
        kv_mount_path=args.kv_mount_path,
        approle_name=args.approle_name,
        approle_policy_name=args.approle_policy_name,
        approle_policy_path=args.approle_policy_path,
        key_shares=args.key_shares,
        key_threshold=args.key_threshold,
        token_ttl=args.token_ttl,
        token_max_ttl=args.token_max_ttl,
        secret_id_ttl=args.secret_id_ttl,
        rotate_secret_id=args.rotate_secret_id,
        ca_certificate=args.ca_certificate,
    )


def main(argv: list[str] | None = None) -> int:
    """Entry point for command-line execution."""

    config = parse_args(argv)
    try:
        state = bootstrap(config)
    except VaultBootstrapError as exc:
        print(f"error: {exc}")
        return 1
    print("Vault appliance bootstrap complete.")
    if state.approle_role_id and state.approle_secret_id:
        print("AppRole credentials available in the state file for downstream use.")
    return 0


if __name__ == "__main__":  # pragma: no cover - CLI entrypoint
    raise SystemExit(main())
