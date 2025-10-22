"""Command helpers for interacting with the Vault appliance."""

from __future__ import annotations

import json
import os
import time
from collections.abc import Iterable, Iterator
from dataclasses import dataclass
from typing import Any

from plumbum import local
from plumbum.commands.processes import ProcessExecutionError

from ._vault_state import (
    VaultBootstrapConfig,
    VaultBootstrapError,
    VaultBootstrapState,
)


@dataclass(slots=True)
class CommandContext:
    """Execution options for :func:`run_command`."""

    env: dict[str, str] | None = None
    stdin: str | None = None
    timeout: int | None = None


def run_command(
    command: str,
    *args: str,
    context: CommandContext | None = None,
) -> str:
    """Execute an external command and return its standard output.

    Examples
    --------
    >>> from pathlib import Path; cfg = VaultBootstrapConfig("https://vault", "tag", Path("state.json")); run_command('printf', 'hello')
    'hello'
    """

    ctx = context or CommandContext()
    bound = local[command][list(args)]
    try:
        if ctx.stdin is None:
            _, stdout, _ = bound.run(env=ctx.env, timeout=ctx.timeout)
        else:
            _, stdout, _ = (bound << ctx.stdin).run(env=ctx.env, timeout=ctx.timeout)
    except ProcessExecutionError as exc:  # pragma: no cover - surface error
        msg = f"Command {command!r} failed: {exc.stderr.strip()}"
        raise VaultBootstrapError(msg) from exc
    return stdout


def build_vault_env(config: VaultBootstrapConfig, token: str | None) -> dict[str, str]:
    """Construct the environment for Vault CLI invocations.

    Examples
    --------
    >>> from pathlib import Path; cfg = VaultBootstrapConfig("https://vault", "tag", Path("state.json")); build_vault_env(cfg, token="root")["VAULT_TOKEN"]
    'root'
    """

    env = os.environ.copy()
    env["VAULT_ADDR"] = config.vault_addr
    if config.ca_certificate is not None:
        env["VAULT_CACERT"] = str(config.ca_certificate)
    if token is not None:
        env["VAULT_TOKEN"] = token
    return env


def _load_droplets(tag: str) -> list[Any]:
    """Return the JSON payload describing Droplets for *tag*.

    Examples
    --------
    >>> from cmd_mox import CmdMox
    >>> with CmdMox() as mox:
    ...     _ = mox.stub('doctl').returns(stdout='[{"networks": {"v4": []}}]')
    ...     mox.replay(); _load_droplets('vault-dev')[0]['networks']
    {'v4': []}
    """

    stdout = run_command(
        "doctl",
        "compute",
        "droplet",
        "list",
        "--tag-name",
        tag,
        "--output",
        "json",
    )
    try:
        droplets = json.loads(stdout)
    except json.JSONDecodeError as exc:  # pragma: no cover - defensive guard
        msg = f"doctl returned invalid JSON for tag {tag!r}: {exc}"
        raise VaultBootstrapError(msg) from exc
    if not isinstance(droplets, list):
        msg = "doctl JSON root must be a list"
        raise VaultBootstrapError(msg)
    return droplets


def _iter_public_ipv4(droplets: Iterable[dict[str, Any]]) -> Iterator[str]:
    """Yield deduplicated public IPv4 addresses from *droplets*.

    Examples
    --------
    >>> list(_iter_public_ipv4([
    ...     {"networks": {"v4": [
    ...         {"type": "public", "ip_address": "203.0.113.10"},
    ...         {"type": "private", "ip_address": "10.0.0.5"},
    ...     ]}}
    ... ]))
    ['203.0.113.10']
    """

    seen: dict[str, None] = {}
    for droplet in droplets:
        networks = droplet.get("networks", {})
        for interface in networks.get("v4", []):
            if interface.get("type") != "public":
                continue
            ip = interface.get("ip_address")
            if not ip or ip in seen:
                continue
            seen[ip] = None
            yield ip


def collect_droplet_ips(tag: str) -> list[str]:
    """Return public IPv4 addresses for Droplets tagged with *tag*.

    Examples
    --------
    >>> from cmd_mox import CmdMox; with CmdMox() as mox: ...
    ...     _ = mox.stub('doctl').returns(stdout='[{"networks":{"v4":[{"type":"public","ip_address":"203.0.113.10"}]}}]'); mox.replay(); collect_droplet_ips('vault-dev')
    ['203.0.113.10']
    """

    droplets = _load_droplets(tag)
    addresses = list(_iter_public_ipv4(droplets))
    if not addresses:
        msg = f"No public IPv4 addresses found for Droplets tagged {tag!r}"
        raise VaultBootstrapError(msg)
    return addresses


def _probe_vault_service(address: str, ssh_args: list[str]) -> None:
    """Assert the Vault systemd unit reports as active for *address*.

    Examples
    --------
    >>> from cmd_mox import CmdMox
    >>> ssh_args = ['-o', 'BatchMode=yes', '-o', 'StrictHostKeyChecking=accept-new',
    ...             '-o', 'ConnectTimeout=10', 'root@203.0.113.10',
    ...             'systemctl', 'is-active', 'vault']
    >>> with CmdMox() as mox:
    ...     mox.stub('ssh').with_args(*ssh_args).returns(stdout='active\n')
    ...     mox.replay(); _probe_vault_service('203.0.113.10', ssh_args)
    """

    last_error: VaultBootstrapError | None = None
    for attempt in range(3):
        try:
            output = run_command(
                "ssh",
                *ssh_args,
                context=CommandContext(timeout=30),
            ).strip()
        except VaultBootstrapError as exc:
            last_error = exc
        else:
            if output == "active":
                return
            msg = (
                "Vault service on {address} is not active "
                "(reported: {output!r})"
            )
            last_error = VaultBootstrapError(msg.format(address=address, output=output))
        if attempt < 2:
            time.sleep(2)
    raise last_error or VaultBootstrapError(
        f"Vault service on {address} did not report as active"
    )


def verify_vault_service(addresses: Iterable[str], config: VaultBootstrapConfig) -> None:
    """Verify the Vault systemd unit is active on each droplet.

    Examples
    --------
    >>> from cmd_mox import CmdMox; from pathlib import Path; cfg = VaultBootstrapConfig('https://vault','tag', Path('state.json'))
    >>> with CmdMox() as mox:
    ...     mox.stub('ssh').with_args('-o','BatchMode=yes','-o','StrictHostKeyChecking=accept-new','-o','ConnectTimeout=10','root@203.0.113.10','systemctl','is-active','vault').returns(stdout='active\n'); mox.replay(); verify_vault_service(['203.0.113.10'], cfg)
    """

    ssh_args_base = [
        "-o",
        "BatchMode=yes",
        "-o",
        "StrictHostKeyChecking=accept-new",
        "-o",
        "ConnectTimeout=10",
    ]
    if config.ssh_identity is not None:
        ssh_args_base.extend(["-i", str(config.ssh_identity)])
    for address in addresses:
        target = f"{config.ssh_user}@{address}"
        ssh_args = [*ssh_args_base, target, "systemctl", "is-active", "vault"]
        _probe_vault_service(address, ssh_args)


def fetch_vault_status(env: dict[str, str]) -> dict[str, Any]:
    """Return the parsed output of ``vault status``.

    Examples
    --------
    >>> from cmd_mox import CmdMox; with CmdMox() as mox: ...
    ...     _ = mox.stub('vault').with_args('status','-format=json').returns(stdout='{"initialized": true, "sealed": false}'); mox.replay(); fetch_vault_status({})['sealed']
    False
    """

    stdout = run_command(
        "vault",
        "status",
        "-format=json",
        context=CommandContext(env=env),
    )
    try:
        return json.loads(stdout)
    except json.JSONDecodeError as exc:
        raise VaultBootstrapError(f"Invalid JSON from vault status: {exc}") from exc


def initialise_vault(config: VaultBootstrapConfig, env: dict[str, str]) -> VaultBootstrapState:
    """Initialise Vault and return the captured bootstrap material.

    Examples
    --------
    >>> from cmd_mox import CmdMox; from pathlib import Path; cfg = VaultBootstrapConfig('https://vault','tag', Path('state.json'))
    >>> with CmdMox() as mox:
    ...     mox.stub('vault').with_args('operator','init','-key-shares','5','-key-threshold','3','-format=json').returns(stdout='{"unseal_keys_b64": ["k1"], "root_token": "root"}'); mox.replay(); initialise_vault(cfg, {}).root_token
    'root'
    """

    stdout = run_command(
        "vault",
        "operator",
        "init",
        "-key-shares",
        str(config.key_shares),
        "-key-threshold",
        str(config.key_threshold),
        "-format=json",
        context=CommandContext(env=env),
    )
    try:
        payload = json.loads(stdout)
    except json.JSONDecodeError as exc:
        raise VaultBootstrapError(f"Invalid JSON from operator init: {exc}") from exc
    unseal_keys = payload.get("unseal_keys_b64")
    root_token = payload.get("root_token")
    if not isinstance(unseal_keys, list) or root_token is None:
        msg = "vault operator init did not return unseal keys and root token"
        raise VaultBootstrapError(msg)
    state = VaultBootstrapState()
    state.update_from_init(unseal_keys, root_token)
    return state


def unseal_vault(env: dict[str, str], state: VaultBootstrapState) -> None:
    """Unseal Vault using the stored key shares.

    Examples
    --------
    >>> from cmd_mox import CmdMox, Response; state = VaultBootstrapState(unseal_keys=['k1'])
    >>> with CmdMox() as mox:
    ...     mox.stub('vault').runs(lambda invocation: Response(stdout='{"sealed": false}')); mox.replay(); unseal_vault({}, state)
    """

    if not state.unseal_keys:
        msg = "No unseal keys recorded; cannot unseal Vault"
        raise VaultBootstrapError(msg)
    for key in state.unseal_keys:
        stdout = run_command(
            "vault",
            "operator",
            "unseal",
            "-format=json",
            key,
            context=CommandContext(env=env),
        )
        try:
            payload = json.loads(stdout)
        except json.JSONDecodeError as exc:
            msg = f"Invalid JSON from operator unseal: {exc}"
            raise VaultBootstrapError(msg) from exc
        if not payload.get("sealed", True):
            break
    else:
        msg = "Vault remains sealed after applying all key shares"
        raise VaultBootstrapError(msg)


def _validate_kv_mount(mount_key: str, existing: dict[str, Any]) -> None:
    """Validate that an existing mount is a KV v2 engine."""

    if existing.get("type") != "kv":
        msg = f"Existing mount at {mount_key} is not a KV engine"
        raise VaultBootstrapError(msg)
    options = existing.get("options", {})
    if options.get("version") != "2":
        msg = f"Existing mount at {mount_key} is not KV v2"
        raise VaultBootstrapError(msg)


def ensure_kv_engine(config: VaultBootstrapConfig, env: dict[str, str]) -> None:
    """Enable the KV v2 secrets engine when missing.

    Examples
    --------
    >>> from cmd_mox import CmdMox, Response; from pathlib import Path; cfg = VaultBootstrapConfig('https://vault','tag', Path('state.json'))
    >>> with CmdMox() as mox:
    ...     mox.stub('vault').runs(lambda inv: Response(stdout='{}') if inv.args[:3]==['secrets','list','-format=json'] else Response(stdout='')); mox.replay(); ensure_kv_engine(cfg, {})
    """

    stdout = run_command(
        "vault",
        "secrets",
        "list",
        "-format=json",
        context=CommandContext(env=env),
    )
    try:
        mounts = json.loads(stdout)
    except json.JSONDecodeError as exc:
        raise VaultBootstrapError(f"Invalid JSON from secrets list: {exc}") from exc
    mount_key = f"{config.kv_mount_path.rstrip('/')}/"
    existing = mounts.get(mount_key)
    if existing is None:
        run_command(
            "vault",
            "secrets",
            "enable",
            f"-path={config.kv_mount_path}",
            "-version=2",
            "kv",
            context=CommandContext(env=env),
        )
        return
    _validate_kv_mount(mount_key, existing)


def _default_policy(config: VaultBootstrapConfig) -> str:
    """Return the default AppRole policy matching the KV mount path.

    Examples
    --------
    >>> from pathlib import Path; cfg = VaultBootstrapConfig('https://vault','tag', Path('state.json')); 'secret/data' in _default_policy(cfg)
    True
    """

    mount = config.kv_mount_path.rstrip("/")
    data_path = f"{mount}/data/*"
    metadata_path = f"{mount}/metadata/*"
    return (
        "\n".join(
            [
                f"path \"{data_path}\" {{",
                '  capabilities = ["create", "read", "update", "list"]',
                "}",
                "",
                f"path \"{metadata_path}\" {{",
                '  capabilities = ["read", "list", "delete"]',
                "}",
                "",
            ]
        )
        + "\n"
    )


def _ensure_approle_auth_enabled(env: dict[str, str]) -> None:
    """Ensure the AppRole auth method is enabled in Vault.

    Examples
    --------
    >>> from cmd_mox import CmdMox, Response; calls = []
    >>> with CmdMox() as mox:
    ...     mox.stub('vault').runs(lambda inv: calls.append(inv.args) or Response(stdout='{}' if inv.args[:3] == ('auth','list','-format=json') else ''))
    ...     mox.replay(); _ensure_approle_auth_enabled({})
    >>> any(args[:2] == ('auth', 'enable') for args in calls)
    True
    """

    stdout = run_command(
        "vault",
        "auth",
        "list",
        "-format=json",
        context=CommandContext(env=env),
    )
    try:
        mounts = json.loads(stdout)
    except json.JSONDecodeError as exc:
        raise VaultBootstrapError(f"Invalid JSON from auth list: {exc}") from exc
    if "approle/" not in mounts:
        run_command(
            "vault",
            "auth",
            "enable",
            "approle",
            context=CommandContext(env=env),
        )


def _write_approle_policy(config: VaultBootstrapConfig, env: dict[str, str]) -> None:
    """Write the AppRole policy to Vault, loading overrides when configured.

    Examples
    --------
    >>> from cmd_mox import CmdMox, Response; from pathlib import Path
    >>> cfg = VaultBootstrapConfig('https://vault','tag', Path('state.json')); captured = {}
    >>> with CmdMox() as mox:
    ...     mox.stub('vault').runs(lambda inv: captured.setdefault('stdin', inv.stdin) or Response(stdout=''))
    ...     mox.replay(); _write_approle_policy(cfg, {})
    >>> bool(captured['stdin'])
    True
    """

    policy_content = (
        config.approle_policy_path.read_text(encoding="utf-8")
        if config.approle_policy_path is not None
        else _default_policy(config)
    )
    run_command(
        "vault",
        "policy",
        "write",
        config.approle_policy_name,
        "-",
        context=CommandContext(env=env, stdin=policy_content),
    )


def _configure_approle_role(config: VaultBootstrapConfig, env: dict[str, str]) -> None:
    """Configure the AppRole with the desired policies and TTL values.

    Examples
    --------
    >>> from cmd_mox import CmdMox, Response; from pathlib import Path
    >>> cfg = VaultBootstrapConfig('https://vault','tag', Path('state.json')); seen = []
    >>> with CmdMox() as mox:
    ...     mox.stub('vault').runs(lambda inv: seen.append(inv.args[:2]) or Response(stdout=''))
    ...     mox.replay(); _configure_approle_role(cfg, {})
    >>> ('write', f'auth/approle/role/{cfg.approle_name}') in seen
    True
    """

    role_path = f"auth/approle/role/{config.approle_name}"
    run_command(
        "vault",
        "write",
        role_path,
        f"token_policies={config.approle_policy_name}",
        f"secret_id_ttl={config.secret_id_ttl}",
        f"token_ttl={config.token_ttl}",
        f"token_max_ttl={config.token_max_ttl}",
        "token_num_uses=0",
        context=CommandContext(env=env),
    )


def _fetch_role_id(config: VaultBootstrapConfig, env: dict[str, str]) -> str | None:
    """Return the AppRole role_id if Vault reports one.

    Examples
    --------
    >>> from cmd_mox import CmdMox, Response; from pathlib import Path
    >>> cfg = VaultBootstrapConfig('https://vault','tag', Path('state.json'))
    >>> with CmdMox() as mox:
    ...     mox.stub('vault').runs(lambda _: Response(stdout='role-id\n'))
    ...     mox.replay(); _fetch_role_id(cfg, {})
    'role-id'
    """

    role_path = f"auth/approle/role/{config.approle_name}"
    stdout = run_command(
        "vault",
        "read",
        "-field=role_id",
        f"{role_path}/role-id",
        context=CommandContext(env=env),
    )
    role_id = stdout.strip()
    return role_id or None


def _generate_secret_id(config: VaultBootstrapConfig, env: dict[str, str]) -> str:
    """Generate and return a fresh AppRole secret_id.

    Examples
    --------
    >>> from cmd_mox import CmdMox, Response; from pathlib import Path
    >>> cfg = VaultBootstrapConfig('https://vault','tag', Path('state.json'))
    >>> with CmdMox() as mox:
    ...     mox.stub('vault').runs(lambda _: Response(stdout='{"data": {"secret_id": "secret"}}'))
    ...     mox.replay(); _generate_secret_id(cfg, {})
    'secret'
    """

    role_path = f"auth/approle/role/{config.approle_name}"
    stdout = run_command(
        "vault",
        "write",
        "-force",
        "-format=json",
        f"{role_path}/secret-id",
        context=CommandContext(env=env),
    )
    try:
        payload = json.loads(stdout)
    except json.JSONDecodeError as exc:
        raise VaultBootstrapError(f"Invalid JSON from secret-id write: {exc}") from exc
    secret_id = payload.get("data", {}).get("secret_id")
    if secret_id is None:
        raise VaultBootstrapError("Failed to retrieve secret_id from Vault")
    return secret_id


def ensure_approle(config: VaultBootstrapConfig, env: dict[str, str], state: VaultBootstrapState) -> None:
    """Provision the DOKS AppRole and capture its credentials.

    Examples
    --------
    >>> from cmd_mox import CmdMox, Response; from pathlib import Path
    >>> cfg = VaultBootstrapConfig('https://vault','tag', Path('state.json')); state = VaultBootstrapState(root_token='root')
    >>> def handler(invocation):
    ...     args = invocation.args
    ...     if args[:3] == ['auth','list','-format=json']: return Response(stdout='{}')
    ...     if args[:2] == ['auth','enable']: return Response(stdout='')
    ...     if args[:2] == ['policy','write']: return Response(stdout='')
    ...     if args[0:2] == ['write', f'auth/approle/role/{cfg.approle_name}']: return Response(stdout='')
    ...     if args[:2] == ['read','-field=role_id']: return Response(stdout='role-id\n')
    ...     if args[:3] == ['write','-force','-format=json']: return Response(stdout='{"data": {"secret_id": "secret"}}')
    ...     raise AssertionError(args)
    >>> with CmdMox() as mox:
    ...     mox.stub('vault').runs(handler); mox.replay(); ensure_approle(cfg, {}, state)
    >>> state.approle_secret_id
    'secret'
    """

    _ensure_approle_auth_enabled(env)
    _write_approle_policy(config, env)
    _configure_approle_role(config, env)

    state.approle_role_id = _fetch_role_id(config, env)

    should_rotate = config.rotate_secret_id or not state.approle_secret_id
    if should_rotate:
        state.approle_secret_id = _generate_secret_id(config, env)


__all__ = [
    "CommandContext",
    "_configure_approle_role",
    "_default_policy",
    "_ensure_approle_auth_enabled",
    "_fetch_role_id",
    "_generate_secret_id",
    "_iter_public_ipv4",
    "_load_droplets",
    "_probe_vault_service",
    "_validate_kv_mount",
    "_write_approle_policy",
    "build_vault_env",
    "collect_droplet_ips",
    "ensure_approle",
    "ensure_kv_engine",
    "fetch_vault_status",
    "initialise_vault",
    "run_command",
    "unseal_vault",
    "verify_vault_service",
]
