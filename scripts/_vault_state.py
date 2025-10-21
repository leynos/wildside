"""State handling for the Vault appliance bootstrap helper."""

from __future__ import annotations

import json
import os
from collections.abc import Iterable
from contextlib import suppress
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any


@dataclass(slots=True)
class VaultBootstrapState:
    """Persistent bootstrap artefacts captured during the initial run."""

    unseal_keys: list[str] = field(default_factory=list)
    root_token: str | None = None
    approle_role_id: str | None = None
    approle_secret_id: str | None = None

    def to_mapping(self) -> dict[str, Any]:
        """Return a JSON-serialisable mapping.

        Examples
        --------
        >>> VaultBootstrapState(unseal_keys=["k"], root_token="t").to_mapping()
        {'unseal_keys': ['k'], 'root_token': 't', 'approle_role_id': None,
         'approle_secret_id': None}
        """

        return {
            "unseal_keys": list(self.unseal_keys),
            "root_token": self.root_token,
            "approle_role_id": self.approle_role_id,
            "approle_secret_id": self.approle_secret_id,
        }

    def update_from_init(self, keys: Iterable[str], root_token: str) -> None:
        """Persist the generated unseal key shares and root token.

        Examples
        --------
        >>> state = VaultBootstrapState(); state.update_from_init(["key"], "root")
        >>> state.unseal_keys
        ['key']
        """

        self.unseal_keys = list(keys)
        self.root_token = root_token


@dataclass(slots=True)
class VaultBootstrapConfig:
    """Configuration supplied via the CLI."""

    vault_addr: str
    droplet_tag: str
    state_file: Path
    ssh_user: str = "root"
    ssh_identity: Path | None = None
    kv_mount_path: str = "secret"
    approle_name: str = "doks-deployer"
    approle_policy_name: str = "doks-deployer"
    approle_policy_path: Path | None = None
    key_shares: int = 5
    key_threshold: int = 3
    token_ttl: str = "1h"
    token_max_ttl: str = "4h"
    secret_id_ttl: str = "4h"
    rotate_secret_id: bool = False
    ca_certificate: Path | None = None


class VaultBootstrapError(RuntimeError):
    """Raised when bootstrap actions fail."""


def _validate_list_str_field(value: Any, field_name: str) -> list[str]:
    """Validate and return a list[str] field from state payload.

    Examples
    --------
    >>> _validate_list_str_field(["a", "b"], "unseal_keys")
    ['a', 'b']
    >>> _validate_list_str_field([], "unseal_keys")
    []
    """

    if not isinstance(value, list) or not all(isinstance(item, str) for item in value):
        msg = f"State field {field_name!r} must be list[str]"
        raise VaultBootstrapError(msg)
    return list(value)


def _validate_optional_str_field(value: Any, field_name: str) -> str | None:
    """Validate and return an optional string field from state payload.

    Examples
    --------
    >>> _validate_optional_str_field("token", "root_token")
    'token'
    >>> _validate_optional_str_field(None, "root_token") is None
    True
    """

    if value is not None and not isinstance(value, str):
        msg = f"State field {field_name!r} must be str | None"
        raise VaultBootstrapError(msg)
    return value


def load_state(path: Path) -> VaultBootstrapState:
    """Load bootstrap state or return an empty object.

    Examples
    --------
    >>> tmp = Path('state.json'); _ = tmp.write_text('{"root_token": "token"}')
    >>> load_state(tmp).root_token
    'token'
    >>> tmp.unlink(); load_state(tmp).unseal_keys
    []
    """

    if not path.exists():
        return VaultBootstrapState()
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:  # pragma: no cover - defensive guard
        msg = f"Failed to parse state file {path}: {exc}"
        raise VaultBootstrapError(msg) from exc
    state = VaultBootstrapState()
    state.unseal_keys = _validate_list_str_field(
        payload.get("unseal_keys", []), "unseal_keys"
    )
    state.root_token = _validate_optional_str_field(
        payload.get("root_token"), "root_token"
    )
    state.approle_role_id = _validate_optional_str_field(
        payload.get("approle_role_id"), "approle_role_id"
    )
    state.approle_secret_id = _validate_optional_str_field(
        payload.get("approle_secret_id"), "approle_secret_id"
    )
    return state


def save_state(path: Path, state: VaultBootstrapState) -> None:
    """Write *state* to ``path`` atomically."""

    path.parent.mkdir(parents=True, exist_ok=True)
    tmp_path = path.with_suffix(path.suffix + ".tmp")
    payload = json.dumps(state.to_mapping(), indent=2)

    fd = os.open(tmp_path, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as handle:
            handle.write(payload)
            handle.flush()
            os.fsync(handle.fileno())
    except Exception:
        with suppress(FileNotFoundError):
            os.unlink(tmp_path)
        raise

    tmp_path.replace(path)
    os.chmod(path, 0o600)


__all__ = [
    "VaultBootstrapConfig",
    "VaultBootstrapError",
    "VaultBootstrapState",
    "load_state",
    "save_state",
]
