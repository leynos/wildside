"""Bootstrap orchestration for the Vault appliance."""

from __future__ import annotations

from typing import Any

from ._vault_commands import (
    build_vault_env,
    collect_droplet_ips,
    ensure_approle,
    ensure_kv_engine,
    fetch_vault_status,
    initialise_vault,
    run_command,  # re-exported for tests
    unseal_vault,
    verify_vault_service,
)
from ._vault_state import (
    VaultBootstrapConfig,
    VaultBootstrapError,
    VaultBootstrapState,
    load_state,
    save_state,
)

__all__ = [
    "VaultBootstrapConfig",
    "VaultBootstrapError",
    "VaultBootstrapState",
    "bootstrap",
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


def _discover_and_verify(config: VaultBootstrapConfig) -> None:
    """Collect droplet addresses and confirm the Vault service is reachable.

    Examples
    --------
    >>> from cmd_mox import CmdMox
    >>> from pathlib import Path
    >>> cfg = VaultBootstrapConfig('https://vault','tag', Path('state.json'))
    >>> with CmdMox() as mox:
    ...     _ = mox.stub('doctl').returns(stdout='[{"networks":{"v4":[{"type":"public","ip_address":"203.0.113.10"}]}}]')
    ...     _ = mox.stub('ssh').returns(stdout='active\n')
    ...     mox.replay(); _discover_and_verify(cfg)
    """

    addresses = collect_droplet_ips(config.droplet_tag)
    verify_vault_service(addresses, config)


def _ensure_initialized(
    config: VaultBootstrapConfig,
    state: VaultBootstrapState,
    env: dict[str, str],
) -> tuple[VaultBootstrapState, dict[str, Any]]:
    """Ensure Vault is initialised and persist the resulting state.

    Examples
    --------
    >>> from cmd_mox import CmdMox, Response
    >>> from pathlib import Path
    >>> cfg = VaultBootstrapConfig('https://vault','tag', Path('state.json'))
    >>> state = VaultBootstrapState()
    >>> with CmdMox() as mox:
    ...     responses = [
    ...         Response(stdout='{"initialized": false, "sealed": true}'),
    ...         Response(stdout='{"unseal_keys_b64": ["k1"], "root_token": "root"}'),
    ...         Response(stdout='{"initialized": true, "sealed": true}')
    ...     ]
    ...     def handler(invocation):
    ...         args = invocation.args
    ...         if args[:2] == ['status', '-format=json']:
    ...             return responses.pop(0)
    ...         if args[:3] == ['operator', 'init', '-key-shares']:
    ...             return responses.pop(0)
    ...         raise AssertionError(args)
    ...     mox.stub('vault').runs(handler); mox.replay()
    ...     new_state, status = _ensure_initialized(cfg, state, {})
    >>> new_state.root_token
    'root'
    >>> status['initialized']
    True
    """

    status = fetch_vault_status(env)
    if status.get("initialized", False):
        return state, status

    state = initialise_vault(config, env)
    save_state(config.state_file, state)
    refreshed = fetch_vault_status(env)
    return state, refreshed


def _ensure_unsealed(
    config: VaultBootstrapConfig,
    state: VaultBootstrapState,
    env: dict[str, str],
    status: dict[str, Any],
) -> None:
    """Unseal Vault when necessary, validating success along the way.

    Examples
    --------
    >>> from cmd_mox import CmdMox, Response
    >>> from pathlib import Path
    >>> cfg = VaultBootstrapConfig('https://vault','tag', Path('state.json'))
    >>> state = VaultBootstrapState(unseal_keys=['k1'])
    >>> with CmdMox() as mox:
    ...     calls = [
    ...         Response(stdout='{"initialized": true, "sealed": true}'),
    ...         Response(stdout='{"sealed": false}'),
    ...         Response(stdout='{"initialized": true, "sealed": false}')
    ...     ]
    ...     def handler(invocation):
    ...         if invocation.args[:2] == ['status','-format=json']:
    ...             return calls.pop(0)
    ...         if invocation.args[:3] == ['operator','unseal','-format=json']:
    ...             return calls.pop(0)
    ...         raise AssertionError(invocation.args)
    ...     mox.stub('vault').runs(handler); mox.replay()
    ...     status = {'sealed': True}
    ...     _ensure_unsealed(cfg, state, {}, status); status['sealed']
    False
    """

    if not status.get("sealed", False):
        return
    if not state.unseal_keys:
        raise VaultBootstrapError(
            "Vault is sealed but no unseal keys are recorded in the state file"
        )

    unseal_vault(env, state)
    refreshed = fetch_vault_status(env)
    status.clear()
    status.update(refreshed)
    if status.get("sealed", False):
        msg = "Vault remains sealed after unseal attempts"
        raise VaultBootstrapError(msg)


def _configure_vault(
    config: VaultBootstrapConfig,
    state: VaultBootstrapState,
    env: dict[str, str],
) -> None:
    """Enable the KV engine and ensure the AppRole is provisioned.

    Examples
    --------
    >>> from cmd_mox import CmdMox, Response
    >>> from pathlib import Path
    >>> cfg = VaultBootstrapConfig('https://vault','tag', Path('state.json'))
    >>> state = VaultBootstrapState(root_token='root')
    >>> with CmdMox() as mox:
    ...     responses = {
    ...         ('secrets','list','-format=json'): Response(stdout='{}'),
    ...         ('auth','list','-format=json'): Response(stdout='{}'),
    ...     }
    ...     def handler(invocation):
    ...         key = tuple(invocation.args)
    ...         return responses.get(key, Response(stdout=''))
    ...     mox.stub('vault').runs(handler); mox.replay(); _configure_vault(cfg, state, {})
    """

    ensure_kv_engine(config, env)
    ensure_approle(config, env, state)


def bootstrap(config: VaultBootstrapConfig) -> VaultBootstrapState:
    """Execute the bootstrap workflow and return the resulting state.

    Examples
    --------
    >>> from cmd_mox import CmdMox, Response; from pathlib import Path
    >>> cfg = VaultBootstrapConfig('https://vault','tag', Path('state.json'))
    >>> state_outputs = {'status': [Response(stdout='{"initialized": false, "sealed": true}'), Response(stdout='{"initialized": true, "sealed": true}'), Response(stdout='{"initialized": true, "sealed": false}')]} 
    >>> def handler(invocation):
    ...     args = invocation.args
    ...     if args == ['status','-format=json']: return state_outputs['status'].pop(0)
    ...     if args[:3] == ['operator','init','-key-shares']: return Response(stdout='{"unseal_keys_b64": ["k1"], "root_token": "root"}')
    ...     if args[:3] == ['operator','unseal','-format=json']: return Response(stdout='{"sealed": false}')
    ...     if args[:3] == ['secrets','list','-format=json']: return Response(stdout='{}')
    ...     return Response(stdout='')
    >>> with CmdMox() as mox:
    ...     _ = mox.stub('doctl').returns(stdout='[{"networks":{"v4":[{"type":"public","ip_address":"203.0.113.10"}]}}]'); _ = mox.stub('ssh').returns(stdout='active\n'); _ = mox.stub('vault').runs(handler); mox.replay(); bootstrap(cfg).root_token
    'root'
    """

    _discover_and_verify(config)

    state = load_state(config.state_file)
    env_without_token = build_vault_env(config, token=None)
    state, status = _ensure_initialized(config, state, env_without_token)
    _ensure_unsealed(config, state, env_without_token, status)

    if state.root_token is None:
        msg = "Missing root token in state; cannot continue"
        raise VaultBootstrapError(msg)

    env_with_token = build_vault_env(config, token=state.root_token)
    _configure_vault(config, state, env_with_token)
    save_state(config.state_file, state)
    return state
