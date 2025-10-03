#!/usr/bin/env -S uv run python
# /// script
# requires-python = ">=3.13"
# dependencies = ["plumbum"]
# ///

"""Bootstrap the DigitalOcean-hosted Vault appliance.

The helper discovers the droplet via tag lookup, verifies Vault is running,
initialises and unseals the cluster, enables the KV v2 secrets engine, and
provisions the DOKS AppRole while storing credentials in DigitalOcean Secrets
Manager. It is deliberately idempotent so reruns converge existing
deployments."""

from __future__ import annotations

import argparse
import json
import sys
from dataclasses import dataclass
from pathlib import Path
from tempfile import TemporaryDirectory
from typing import Iterable, Sequence

from plumbum import ProcessExecutionError, local


@dataclass(frozen=True)
class BootstrapOptions:
    """Configuration derived from command line arguments."""

    environment: str
    droplet_tag: str
    ssh_user: str
    mount_path: str
    approle_name: str
    policy_name: str
    secret_prefix: str
    key_shares: int
    key_threshold: int
    vault_address: str | None = None
    ca_cert_path: str | None = None


class CommandRunner:
    """Thin wrapper around :mod:`plumbum` for deterministic command execution."""

    def __init__(self, local_module=local):
        self._local = local_module

    def run(self, command: str, *args: str, env: dict[str, str] | None = None) -> str:
        cmd = self._local[command]
        if env:
            cmd = cmd.with_env(**env)
        if args:
            cmd = cmd[args]
        return cmd()


class SecretStore:
    """DigitalOcean Secrets Manager helper around ``doctl`` commands."""

    def __init__(self, runner: CommandRunner, prefix: str):
        self._runner = runner
        self._prefix = prefix

    def _name(self, suffix: str) -> str:
        return f"{self._prefix}-{suffix}"

    def get(self, suffix: str) -> str | None:
        name = self._name(suffix)
        try:
            output = self._runner.run(
                "doctl",
                "secrets",
                "manager",
                "secrets",
                "get",
                name,
                "--output",
                "json",
            )
        except ProcessExecutionError as exc:
            if exc.retcode == 1:
                return None
            raise
        payload = json.loads(output)
        # ``doctl`` returns ``{"secret": {"value": "..."}}`` for ``--output json``
        try:
            return payload["secret"]["value"]
        except (KeyError, TypeError) as err:
            raise RuntimeError(f"Unexpected secret payload for {name}") from err

    def put(self, suffix: str, value: str) -> None:
        name = self._name(suffix)
        try:
            self._runner.run(
                "doctl",
                "secrets",
                "manager",
                "secrets",
                "create",
                name,
                "--data",
                value,
            )
        except ProcessExecutionError as exc:
            if exc.retcode != 10:
                raise
            # ``create`` exits 10 when the secret already exists. Update instead.
            self._runner.run(
                "doctl",
                "secrets",
                "manager",
                "secrets",
                "update",
                name,
                "--data",
                value,
            )


def parse_args(argv: Sequence[str] | None = None) -> BootstrapOptions:
    parser = argparse.ArgumentParser(description="Bootstrap the Vault appliance")
    parser.add_argument(
        "--environment",
        required=True,
        help="Logical environment identifier (for example 'dev').",
    )
    parser.add_argument(
        "--droplet-tag",
        required=True,
        help="DigitalOcean tag used to discover the Vault droplet.",
    )
    parser.add_argument(
        "--ssh-user",
        default="root",
        help="SSH user with access to the appliance (default: root).",
    )
    parser.add_argument(
        "--mount-path",
        default="secret",
        help="KV v2 mount point used for application secrets (default: secret).",
    )
    parser.add_argument(
        "--approle-name",
        default="doks-deployer",
        help="Name of the AppRole consumed by the DOKS workflow.",
    )
    parser.add_argument(
        "--policy-name",
        default="doks-deployer",
        help="Name of the policy bound to the DOKS AppRole.",
    )
    parser.add_argument(
        "--secret-prefix",
        required=True,
        help="Prefix for DigitalOcean Secrets that store unseal material.",
    )
    parser.add_argument(
        "--key-shares",
        type=int,
        default=5,
        help="Number of unseal key shares to generate when initialising Vault.",
    )
    parser.add_argument(
        "--key-threshold",
        type=int,
        default=3,
        help="Number of unseal shares required to unseal Vault.",
    )
    parser.add_argument(
        "--vault-address",
        help=(
            "Override the Vault API address. Defaults to the discovered droplet"
            " IP on port 8200 over HTTPS."
        ),
    )
    parser.add_argument(
        "--ca-cert-path",
        help=(
            "Optional path to the Vault certificate authority bundle. When set,"
            " exported as VAULT_CACERT for CLI calls."
        ),
    )
    args = parser.parse_args(argv)
    return BootstrapOptions(
        environment=args.environment,
        droplet_tag=args.droplet_tag,
        ssh_user=args.ssh_user,
        mount_path=args.mount_path.rstrip("/"),
        approle_name=args.approle_name,
        policy_name=args.policy_name,
        secret_prefix=args.secret_prefix,
        key_shares=args.key_shares,
        key_threshold=args.key_threshold,
        vault_address=args.vault_address.rstrip("/") if args.vault_address else None,
        ca_cert_path=args.ca_cert_path,
    )


def discover_droplet_ip(options: BootstrapOptions, runner: CommandRunner) -> str:
    output = runner.run(
        "doctl",
        "compute",
        "droplet",
        "list",
        "--tag-name",
        options.droplet_tag,
        "--format",
        "PublicIPv4",
        "--no-header",
    )
    addresses = [line.strip() for line in output.splitlines() if line.strip()]
    if not addresses:
        raise RuntimeError(
            f"No droplets tagged '{options.droplet_tag}' found in DigitalOcean."
        )
    if len(addresses) > 1:
        raise RuntimeError(
            "Multiple droplets matched the Vault tag; aborting to avoid ambiguity."
        )
    return addresses[0]


def verify_vault_service(ip: str, options: BootstrapOptions, runner: CommandRunner) -> None:
    status = runner.run(
        "ssh",
        f"{options.ssh_user}@{ip}",
        "sudo",
        "systemctl",
        "is-active",
        "vault",
    ).strip()
    if status != "active":
        raise RuntimeError(f"Vault systemd unit is not active (reported '{status}').")


def read_vault_status(runner: CommandRunner, env: dict[str, str]) -> dict:
    output = runner.run("vault", "status", "-format=json", env=env)
    return json.loads(output)


def initialise_vault(
    options: BootstrapOptions,
    runner: CommandRunner,
    env: dict[str, str],
    secrets: SecretStore,
) -> dict:
    init_payload = runner.run(
        "vault",
        "operator",
        "init",
        "-key-shares",
        str(options.key_shares),
        "-key-threshold",
        str(options.key_threshold),
        "-format=json",
        env=env,
    )
    init_data = json.loads(init_payload)
    unseal_keys: Iterable[str] = init_data.get("unseal_keys_b64", [])
    root_token: str | None = init_data.get("root_token")
    if not root_token:
        raise RuntimeError("Vault did not return a root token during initialisation.")
    for index, key in enumerate(unseal_keys, start=1):
        secrets.put(f"unseal-{index}", key)
    secrets.put("root-token", root_token)
    return {"unseal_keys": list(unseal_keys), "root_token": root_token}


def load_unseal_keys(options: BootstrapOptions, secrets: SecretStore) -> list[str]:
    keys: list[str] = []
    for index in range(1, options.key_shares + 1):
        key = secrets.get(f"unseal-{index}")
        if key:
            keys.append(key)
    return keys


def unseal_vault(
    keys: Sequence[str],
    options: BootstrapOptions,
    runner: CommandRunner,
    env: dict[str, str],
) -> None:
    if len(keys) < options.key_threshold:
        raise RuntimeError(
            "Insufficient unseal keys available; aborting to keep Vault sealed."
        )
    for key in keys[: options.key_threshold]:
        runner.run("vault", "operator", "unseal", key, env=env)


def ensure_kv_mount(options: BootstrapOptions, runner: CommandRunner, env: dict[str, str]) -> None:
    mounts = json.loads(runner.run("vault", "secrets", "list", "-format=json", env=env))
    mount_path = f"{options.mount_path}/"
    current = mounts.get(mount_path)
    if current and current.get("type") == "kv" and current.get("options", {}).get("version") == "2":
        return
    runner.run(
        "vault",
        "secrets",
        "enable",
        "-path",
        options.mount_path,
        "kv-v2",
        env=env,
    )


def ensure_approle(
    options: BootstrapOptions,
    runner: CommandRunner,
    env: dict[str, str],
    secrets: SecretStore,
) -> None:
    auth_methods = json.loads(
        runner.run("vault", "auth", "list", "-format=json", env=env)
    )
    if "approle/" not in auth_methods:
        runner.run("vault", "auth", "enable", "approle", env=env)

    policy_hcl = (
        f'path "{options.mount_path}/data/*" {{\n'
        "  capabilities = [\"read\", \"list\"]\n"
        "}\n"
    )
    with TemporaryDirectory(prefix="vault-policy-") as tempdir:
        policy_path = Path(tempdir, f"{options.policy_name}.hcl")
        policy_path.write_text(policy_hcl, encoding="utf-8")
        runner.run(
            "vault",
            "policy",
            "write",
            options.policy_name,
            str(policy_path),
            env=env,
        )

    runner.run(
        "vault",
        "write",
        f"auth/approle/role/{options.approle_name}",
        f"token_policies={options.policy_name}",
        "secret_id_ttl=24h",
        "token_ttl=1h",
        "token_max_ttl=4h",
        env=env,
    )

    role_id = runner.run(
        "vault",
        "read",
        "-field=role_id",
        f"auth/approle/role/{options.approle_name}/role-id",
        env=env,
    ).strip()
    secret_id = runner.run(
        "vault",
        "write",
        "-f",
        "-field=secret_id",
        f"auth/approle/role/{options.approle_name}/secret-id",
        env=env,
    ).strip()

    secrets.put("role-id", role_id)
    secrets.put("secret-id", secret_id)


def bootstrap(options: BootstrapOptions, runner: CommandRunner | None = None) -> None:
    command_runner = runner or CommandRunner()
    secrets = SecretStore(command_runner, options.secret_prefix)

    ip_address = discover_droplet_ip(options, command_runner)
    verify_vault_service(ip_address, options, command_runner)

    address = options.vault_address or f"https://{ip_address}:8200"
    vault_env: dict[str, str] = {"VAULT_ADDR": address}
    if options.ca_cert_path:
        vault_env["VAULT_CACERT"] = options.ca_cert_path
    status = read_vault_status(command_runner, vault_env)

    if not status.get("initialized", False):
        init_data = initialise_vault(options, command_runner, vault_env, secrets)
        unseal_vault(init_data["unseal_keys"], options, command_runner, vault_env)
        vault_env["VAULT_TOKEN"] = init_data["root_token"]
    else:
        if status.get("sealed", False):
            unseal_keys = load_unseal_keys(options, secrets)
            unseal_vault(unseal_keys, options, command_runner, vault_env)
        root_token = secrets.get("root-token")
        if not root_token:
            raise RuntimeError(
                "Vault is initialised but no root token is stored; cannot proceed."
            )
        vault_env["VAULT_TOKEN"] = root_token

    post_status = read_vault_status(command_runner, vault_env)
    if post_status.get("sealed", False):
        raise RuntimeError("Vault remains sealed after attempting to unseal it.")

    ensure_kv_mount(options, command_runner, vault_env)
    ensure_approle(options, command_runner, vault_env, secrets)


def main(argv: Sequence[str] | None = None) -> int:
    try:
        options = parse_args(argv)
        bootstrap(options)
    except Exception as exc:  # noqa: BLE001 - top-level guard
        print(f"Error: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
