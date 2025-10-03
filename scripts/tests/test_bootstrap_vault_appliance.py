from __future__ import annotations

import json
from dataclasses import replace

import pytest

from cmd_mox import CommandRegistry

from scripts.bootstrap_vault_appliance import BootstrapOptions, CommandRunner, bootstrap


def make_options() -> BootstrapOptions:
    return BootstrapOptions(
        environment="dev",
        droplet_tag="vault-dev",
        ssh_user="root",
        mount_path="secret",
        approle_name="doks",
        policy_name="doks",
        secret_prefix="dev-vault",
        key_shares=5,
        key_threshold=3,
        vault_address="https://vault.dev.example:8200",
    )


def test_bootstrap_initialises_and_configures_vault(tmp_path) -> None:
    registry = CommandRegistry()
    doctl = registry.create("doctl")
    vault = registry.create("vault")
    ssh = registry.create("ssh")
    runner = CommandRunner(local_module=registry.local_proxy)

    options = make_options()

    doctl.queue(
        "compute",
        "droplet",
        "list",
        "--tag-name",
        options.droplet_tag,
        "--format",
        "PublicIPv4",
        "--no-header",
        stdout="203.0.113.10\n",
    )
    ssh.queue(
        f"{options.ssh_user}@203.0.113.10",
        "sudo",
        "systemctl",
        "is-active",
        "vault",
        stdout="active\n",
    )
    vault.queue(
        "status",
        "-format=json",
        stdout=json.dumps({"initialized": False, "sealed": True}),
    )
    init_payload = json.dumps(
        {
            "unseal_keys_b64": [
                "key-1",
                "key-2",
                "key-3",
                "key-4",
                "key-5",
            ],
            "root_token": "root-token",
        }
    )
    vault.queue(
        "operator",
        "init",
        "-key-shares",
        str(options.key_shares),
        "-key-threshold",
        str(options.key_threshold),
        "-format=json",
        stdout=init_payload,
    )
    for index in range(1, options.key_shares + 1):
        doctl.queue(
            "secrets",
            "manager",
            "secrets",
            "create",
            f"{options.secret_prefix}-unseal-{index}",
            "--data",
            f"key-{index}",
        )
    doctl.queue(
        "secrets",
        "manager",
        "secrets",
        "create",
        f"{options.secret_prefix}-root-token",
        "--data",
        "root-token",
    )
    for index in range(1, options.key_threshold + 1):
        vault.queue("operator", "unseal", f"key-{index}")
    vault.queue(
        "status",
        "-format=json",
        stdout=json.dumps({"initialized": True, "sealed": False}),
    )
    vault.queue(
        "secrets",
        "list",
        "-format=json",
        stdout=json.dumps({}),
    )
    vault.queue(
        "secrets",
        "enable",
        "-path",
        options.mount_path,
        "kv-v2",
    )
    vault.queue(
        "auth",
        "list",
        "-format=json",
        stdout=json.dumps({}),
    )
    vault.queue("auth", "enable", "approle")
    vault.queue()
    vault.queue(
        "write",
        f"auth/approle/role/{options.approle_name}",
        f"token_policies={options.policy_name}",
        "secret_id_ttl=24h",
        "token_ttl=1h",
        "token_max_ttl=4h",
    )
    vault.queue(
        "read",
        "-field=role_id",
        f"auth/approle/role/{options.approle_name}/role-id",
        stdout="role-123\n",
    )
    vault.queue(
        "write",
        "-f",
        "-field=secret_id",
        f"auth/approle/role/{options.approle_name}/secret-id",
        stdout="secret-456\n",
    )
    doctl.queue(
        "secrets",
        "manager",
        "secrets",
        "create",
        f"{options.secret_prefix}-role-id",
        "--data",
        "role-123",
    )
    doctl.queue(
        "secrets",
        "manager",
        "secrets",
        "create",
        f"{options.secret_prefix}-secret-id",
        "--data",
        "secret-456",
    )

    bootstrap(options, runner=runner)

    policy_calls = [call for call in vault.calls if call.args[:2] == ("policy", "write")]
    assert len(policy_calls) == 1
    assert policy_calls[0].env.get("VAULT_TOKEN") == "root-token"
    assert policy_calls[0].env.get("VAULT_ADDR") == options.vault_address


def test_bootstrap_reuses_existing_configuration() -> None:
    registry = CommandRegistry()
    doctl = registry.create("doctl")
    vault = registry.create("vault")
    ssh = registry.create("ssh")
    runner = CommandRunner(local_module=registry.local_proxy)

    options = replace(make_options(), ca_cert_path="/tmp/vault-ca.pem")

    doctl.queue(
        "compute",
        "droplet",
        "list",
        "--tag-name",
        options.droplet_tag,
        "--format",
        "PublicIPv4",
        "--no-header",
        stdout="203.0.113.10\n",
    )
    ssh.queue(
        f"{options.ssh_user}@203.0.113.10",
        "sudo",
        "systemctl",
        "is-active",
        "vault",
        stdout="active\n",
    )
    vault.queue(
        "status",
        "-format=json",
        stdout=json.dumps({"initialized": True, "sealed": False}),
    )
    doctl.queue(
        "secrets",
        "manager",
        "secrets",
        "get",
        f"{options.secret_prefix}-root-token",
        "--output",
        "json",
        stdout=json.dumps({"secret": {"value": "root-token"}}),
    )
    vault.queue(
        "status",
        "-format=json",
        stdout=json.dumps({"initialized": True, "sealed": False}),
    )
    vault.queue(
        "secrets",
        "list",
        "-format=json",
        stdout=json.dumps(
            {"secret/": {"type": "kv", "options": {"version": "2"}}}
        ),
    )
    vault.queue(
        "auth",
        "list",
        "-format=json",
        stdout=json.dumps({"approle/": {}}),
    )
    vault.queue()
    vault.queue(
        "write",
        f"auth/approle/role/{options.approle_name}",
        f"token_policies={options.policy_name}",
        "secret_id_ttl=24h",
        "token_ttl=1h",
        "token_max_ttl=4h",
    )
    vault.queue(
        "read",
        "-field=role_id",
        f"auth/approle/role/{options.approle_name}/role-id",
        stdout="role-abc\n",
    )
    vault.queue(
        "write",
        "-f",
        "-field=secret_id",
        f"auth/approle/role/{options.approle_name}/secret-id",
        stdout="secret-xyz\n",
    )
    doctl.queue(
        "secrets",
        "manager",
        "secrets",
        "create",
        f"{options.secret_prefix}-role-id",
        "--data",
        "role-abc",
        exit_code=10,
    )
    doctl.queue(
        "secrets",
        "manager",
        "secrets",
        "update",
        f"{options.secret_prefix}-role-id",
        "--data",
        "role-abc",
    )
    doctl.queue(
        "secrets",
        "manager",
        "secrets",
        "create",
        f"{options.secret_prefix}-secret-id",
        "--data",
        "secret-xyz",
        exit_code=10,
    )
    doctl.queue(
        "secrets",
        "manager",
        "secrets",
        "update",
        f"{options.secret_prefix}-secret-id",
        "--data",
        "secret-xyz",
    )

    bootstrap(options, runner=runner)

    init_calls = [call for call in vault.calls if call.args[:2] == ("operator", "init")]
    assert not init_calls
    first_call = vault.calls[0]
    assert first_call.env.get("VAULT_ADDR") == options.vault_address
    assert first_call.env.get("VAULT_CACERT") == "/tmp/vault-ca.pem"


def test_bootstrap_aborts_when_unseal_keys_missing() -> None:
    registry = CommandRegistry()
    doctl = registry.create("doctl")
    vault = registry.create("vault")
    ssh = registry.create("ssh")
    runner = CommandRunner(local_module=registry.local_proxy)

    options = replace(make_options(), vault_address=None)

    doctl.queue(
        "compute",
        "droplet",
        "list",
        "--tag-name",
        options.droplet_tag,
        "--format",
        "PublicIPv4",
        "--no-header",
        stdout="203.0.113.10\n",
    )
    ssh.queue(
        f"{options.ssh_user}@203.0.113.10",
        "sudo",
        "systemctl",
        "is-active",
        "vault",
        stdout="active\n",
    )
    vault.queue(
        "status",
        "-format=json",
        stdout=json.dumps({"initialized": True, "sealed": True}),
    )
    for index in range(1, options.key_shares + 1):
        doctl.queue(
            "secrets",
            "manager",
            "secrets",
            "get",
            f"{options.secret_prefix}-unseal-{index}",
            "--output",
            "json",
            exit_code=1,
        )

    with pytest.raises(RuntimeError, match="Insufficient unseal keys"):
        bootstrap(options, runner=runner)

    assert vault.calls[0].env.get("VAULT_ADDR") == "https://203.0.113.10:8200"
    assert "VAULT_CACERT" not in vault.calls[0].env
