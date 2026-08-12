"""Unit tests for local preview session-secret reconciliation.

These tests exercise ``ensure_session_secret`` and the manifest apply path in
``local_k8s.session_secret`` without contacting a cluster. They document how a
runtime signing key is generated, how an existing key is reused, and how the
concurrent-create race is reconciled when two runs apply the Secret at once.
"""

from __future__ import annotations

import base64
import dataclasses as dc
import typing as typ
from types import SimpleNamespace

import pytest
from conftest import CommandRecord, install_run_recorder
from local_k8s.deployment import ensure_session_secret
from local_k8s.session_secret import _apply_session_secret_manifest
from local_k8s.validation import LocalK8sError

if typ.TYPE_CHECKING:
    import collections.abc as cabc

    from local_k8s.config import PreviewConfig


def test_ensure_session_secret_applies_runtime_key_manifest(
    monkeypatch: pytest.MonkeyPatch,
    preview_config: PreviewConfig,
) -> None:
    """Verify local preview creates a mounted session signing key Secret."""
    commands = install_run_recorder(monkeypatch)

    def deterministic_key(length: int) -> bytes:
        """Return a deterministic key for manifest assertions."""
        assert length == 96, "session key generator must request 96 bytes"
        return b"a" * length

    ensure_session_secret(preview_config, key_generator=deterministic_key)

    assert commands[0] == (
        "kubectl",
        [
            "--context",
            "k3d-wildside-preview",
            "-n",
            "wildside",
            "get",
            "secret",
            "wildside-session-key",
            "--ignore-not-found",
            "-o=jsonpath={.data.session_key}",
        ],
        None,
    ), "local preview must check for an existing session Secret before applying"
    apply_command, apply_args, manifest = commands[1]
    assert apply_command == "kubectl", (
        "local preview must create the session Secret with kubectl"
    )
    assert apply_args == [
        "--context",
        "k3d-wildside-preview",
        "create",
        "-f",
        "-",
    ], "local preview must atomically create the session Secret before Helm"
    assert manifest is not None, (
        "session Secret creation must send the manifest on stdin"
    )
    assert "name: wildside-session-key" in manifest, (
        "session Secret manifest must name the wildside-session-key Secret"
    )
    assert "namespace: wildside" in manifest, (
        "session Secret manifest must target the wildside namespace"
    )
    encoded_key = manifest.rsplit("session_key: ", maxsplit=1)[1].strip()
    assert base64.b64decode(encoded_key) == b"a" * 96, (
        "session Secret manifest must base64-encode the generated signing key"
    )


def test_ensure_session_secret_reuses_existing_key(
    monkeypatch: pytest.MonkeyPatch,
    preview_config: PreviewConfig,
) -> None:
    """Verify local preview does not rotate an existing session signing key."""
    commands = install_run_recorder(
        monkeypatch,
        stdout=base64.b64encode(b"existing-key").decode("ascii"),
    )

    def fail_on_rotation(_length: int) -> bytes:
        error_message = "existing local preview session keys must be reused"
        raise AssertionError(error_message)

    ensure_session_secret(preview_config, key_generator=fail_on_rotation)

    assert commands == [
        (
            "kubectl",
            [
                "--context",
                "k3d-wildside-preview",
                "-n",
                "wildside",
                "get",
                "secret",
                "wildside-session-key",
                "--ignore-not-found",
                "-o=jsonpath={.data.session_key}",
            ],
            None,
        )
    ], "existing local preview session keys must be reused without apply"


def _already_exists_stderr() -> str:
    """Return kubectl stderr carrying the structured AlreadyExists reason."""
    return (
        'Error from server (AlreadyExists): secrets "wildside-session-key" '
        "already exists"
    )


def _already_exists_message() -> str:
    """Return the formatted error message for an AlreadyExists conflict."""
    return 'secrets "wildside-session-key" already exists'


def _is_get_session_secret(args: list[str]) -> bool:
    """Return whether the kubectl args read the session Secret."""
    return args[4:7] == ["get", "secret", "wildside-session-key"]


def _valid_session_key_stdout() -> str:
    """Return a base64 session_key payload for a healthy Secret."""
    return base64.b64encode(b"a" * 96).decode("ascii")


def _validated_input_text(input_text: object) -> str | None:
    """Return `input_text` unchanged, or raise if it is a non-`str` value."""
    if input_text is not None and not isinstance(input_text, str):
        error_message = "input_text must be text when provided"
        raise TypeError(error_message)
    return input_text


@dc.dataclass(slots=True)
class _ConcurrentSecretResponder:
    """Fake `run` simulating a Secret created concurrently by another process."""

    ready_after_get_calls: int
    commands: list[CommandRecord] = dc.field(default_factory=list)

    def __call__(
        self, command: str, args: list[str], **kwargs: object
    ) -> SimpleNamespace:
        input_text = _validated_input_text(kwargs.get("input_text"))
        self.commands.append((command, args, input_text))
        if _is_get_session_secret(args):
            return self._respond_to_get()
        return self._respond_to_write(command, args)

    def _respond_to_write(self, command: str, args: list[str]) -> SimpleNamespace:
        if args[2:5] == ["create", "-f", "-"]:
            raise LocalK8sError(
                _already_exists_message(),
                stderr=_already_exists_stderr(),
            )
        if args[2:5] == ["apply", "-f", "-"]:
            return SimpleNamespace(stdout="")
        error_message = f"unexpected command: {command} {args}"
        raise AssertionError(error_message)

    def _respond_to_get(self) -> SimpleNamespace:
        get_calls = sum(1 for _c, a, _i in self.commands if _is_get_session_secret(a))
        stdout = (
            _valid_session_key_stdout()
            if get_calls >= self.ready_after_get_calls
            else ""
        )
        return SimpleNamespace(stdout=stdout)


def test_ensure_session_secret_reuses_concurrent_create(
    monkeypatch: pytest.MonkeyPatch,
    preview_config: PreviewConfig,
) -> None:
    """Verify a concurrently created Secret with key material is reused."""
    responder = _ConcurrentSecretResponder(ready_after_get_calls=2)
    monkeypatch.setattr("local_k8s.session_secret.run", responder)

    ensure_session_secret(preview_config, key_generator=lambda length: b"a" * length)

    assert len(responder.commands) == 3, (
        "concurrent create reuse must re-fetch and validate the existing Secret"
    )
    assert _is_get_session_secret(responder.commands[-1][1]), (
        "reuse must confirm the concurrently created Secret's key material"
    )
    assert all(cmd[1][2:5] != ["apply", "-f", "-"] for cmd in responder.commands), (
        "a valid concurrent Secret must be reused without re-applying it"
    )


def test_ensure_session_secret_repairs_malformed_concurrent_secret(
    monkeypatch: pytest.MonkeyPatch,
    preview_config: PreviewConfig,
) -> None:
    """Verify a concurrently created Secret without a key is repaired, then reused."""
    responder = _ConcurrentSecretResponder(ready_after_get_calls=3)
    monkeypatch.setattr("local_k8s.session_secret.run", responder)

    ensure_session_secret(preview_config, key_generator=lambda length: b"a" * length)

    apply_commands = [
        cmd for cmd in responder.commands if cmd[1][2:5] == ["apply", "-f", "-"]
    ]
    assert len(apply_commands) == 1, (
        "a malformed concurrent Secret must be repaired with a single apply"
    )
    assert apply_commands[0][2] is not None, (
        "repair must send the fresh session Secret manifest on stdin"
    )


def test_ensure_session_secret_fails_when_secret_stays_malformed(
    monkeypatch: pytest.MonkeyPatch,
    preview_config: PreviewConfig,
) -> None:
    """Verify an unrepairable malformed Secret fails explicitly."""
    # A `ready_after_get_calls` beyond every get keeps the Secret malformed
    # through the initial check, the post-conflict re-fetch, and the re-fetch
    # after the apply-based repair.
    responder = _ConcurrentSecretResponder(ready_after_get_calls=99)
    monkeypatch.setattr("local_k8s.session_secret.run", responder)

    with pytest.raises(LocalK8sError, match="still lacks session_key after repair"):
        ensure_session_secret(
            preview_config, key_generator=lambda length: b"a" * length
        )


def _raise_on_create(
    exc: LocalK8sError,
) -> cabc.Callable[..., SimpleNamespace]:
    """Return a ``run`` replacement that raises ``exc`` on the create call."""

    def _run(command: str, args: list[str], **kwargs: object) -> SimpleNamespace:
        _validated_input_text(kwargs.get("input_text"))
        if args[2:5] == ["create", "-f", "-"]:
            raise exc
        error_message = f"unexpected command: {command} {args}"
        raise AssertionError(error_message)

    return _run


def test_apply_session_secret_reconciles_genuine_conflict(
    monkeypatch: pytest.MonkeyPatch,
    preview_config: PreviewConfig,
) -> None:
    """Verify a genuine AlreadyExists conflict triggers reconciliation."""
    conflict = LocalK8sError(_already_exists_message(), stderr=_already_exists_stderr())
    monkeypatch.setattr("local_k8s.session_secret.run", _raise_on_create(conflict))
    reconciled: list[str] = []
    monkeypatch.setattr(
        "local_k8s.session_secret._reconcile_existing_session_secret",
        lambda _config, manifest: reconciled.append(manifest),
    )

    _apply_session_secret_manifest(preview_config, "manifest-body")

    assert reconciled == ["manifest-body"], (
        "a genuine AlreadyExists server conflict must reconcile the existing Secret"
    )


def test_apply_session_secret_reraises_non_conflict_error(
    monkeypatch: pytest.MonkeyPatch,
    preview_config: PreviewConfig,
) -> None:
    """Verify a non-conflict kubectl failure propagates without reconciling."""
    failure = LocalK8sError(
        "connection refused",
        stderr="Unable to connect to the server: connection refused",
    )
    monkeypatch.setattr("local_k8s.session_secret.run", _raise_on_create(failure))
    reconciled: list[str] = []
    monkeypatch.setattr(
        "local_k8s.session_secret._reconcile_existing_session_secret",
        lambda _config, manifest: reconciled.append(manifest),
    )

    with pytest.raises(LocalK8sError, match="connection refused"):
        _apply_session_secret_manifest(preview_config, "manifest-body")

    assert reconciled == [], (
        "a non-conflict kubectl failure must propagate without reconciling"
    )


def test_apply_session_secret_reraises_incidental_already_exists_message(
    monkeypatch: pytest.MonkeyPatch,
    preview_config: PreviewConfig,
) -> None:
    """Verify an incidental "already exists" message re-raises.

    Only the structured server reason may trigger reconciliation.
    """
    # The message mentions "already exists" but the stderr lacks the structured
    # ``(AlreadyExists)`` server reason, so it must not be treated as a conflict.
    misleading = LocalK8sError(
        'the namespace "already exists" is terminating',
        stderr=(
            'Error from server (Forbidden): namespace "already exists" is terminating'
        ),
    )
    monkeypatch.setattr("local_k8s.session_secret.run", _raise_on_create(misleading))
    reconciled: list[str] = []
    monkeypatch.setattr(
        "local_k8s.session_secret._reconcile_existing_session_secret",
        lambda _config, manifest: reconciled.append(manifest),
    )

    with pytest.raises(LocalK8sError, match="already exists"):
        _apply_session_secret_manifest(preview_config, "manifest-body")

    assert reconciled == [], (
        "an incidental 'already exists' message without the AlreadyExists "
        "server reason must propagate without reconciling"
    )
