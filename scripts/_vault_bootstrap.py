"""Bootstrap orchestration for the Vault appliance."""

from __future__ import annotations

from _vault_commands import (
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
from _vault_state import (
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

    addresses = collect_droplet_ips(config.droplet_tag)
    verify_vault_service(addresses, config)

    state = load_state(config.state_file)
    env_without_token = build_vault_env(config, token=None)
    status = fetch_vault_status(env_without_token)

    if not status.get("initialized", False):
        state = initialise_vault(config, env_without_token)
        save_state(config.state_file, state)
        status = fetch_vault_status(env_without_token)

    if status.get("sealed", False):
        if not state.unseal_keys:
            raise VaultBootstrapError(
                "Vault is sealed but no unseal keys are recorded in the state file"
            )
        unseal_vault(env_without_token, state)
        status = fetch_vault_status(env_without_token)
        if status.get("sealed", False):
            raise VaultBootstrapError("Vault remains sealed after unseal attempts")

    if state.root_token is None:
        raise VaultBootstrapError("Missing root token in state; cannot continue")

    env_with_token = build_vault_env(config, token=state.root_token)
    ensure_kv_engine(config, env_with_token)
    ensure_approle(config, env_with_token, state)
    save_state(config.state_file, state)
    return state
