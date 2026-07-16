"""Unit tests for local preview deployment orchestration.

These tests exercise the orchestration logic in ``local_k8s.deployment``
without invoking Kubernetes, Helm, k3d, or Docker. They document the preflight
contract for full build-and-deploy runs and the ``skip_build`` path used with
prebuilt images. The key invariant is that deployment tools depend on the
selected provider, while Docker or Podman is required only when the deployment
will build an image locally.
"""

from __future__ import annotations

import base64
from collections.abc import Callable
from dataclasses import dataclass, field, replace
from types import SimpleNamespace
from typing import TYPE_CHECKING, cast

import pytest

from local_k8s.config import PreviewConfig
from local_k8s.deployment import (
    _deploy_preview_tools,
    build_image,
    deploy_preview,
    ensure_session_secret,
    helm_upgrade,
)
from local_k8s.session_secret import _apply_session_secret_manifest
from local_k8s.validation import LocalK8sError

from conftest import CommandRecord, install_run_recorder

if TYPE_CHECKING:
    from local_k8s.config import K8sProvider


@pytest.mark.parametrize(
    ("skip_build", "expected_tools"),
    [
        (True, ("helm", "k3d", "kubectl")),
        (False, ("docker", "helm", "k3d", "kubectl")),
    ],
    ids=["skip-build", "build-image"],
)
def test_deploy_preview_docker_requirement_conditional_on_skip_build(
    monkeypatch: pytest.MonkeyPatch,
    preview_config: PreviewConfig,
    skip_build: bool,  # noqa: FBT001 - pytest parametrize documents both boolean modes.
    expected_tools: tuple[str, ...],
) -> None:
    """Verify that Docker preflight follows the selected build mode."""
    required_tools: list[tuple[str, ...]] = []
    calls: list[str] = []

    def record_step(name: str) -> Callable[[PreviewConfig], None]:
        """Return a side-effect replacement that records orchestration order."""

        def step(_: PreviewConfig) -> None:
            calls.append(name)

        return step

    monkeypatch.setattr(
        "local_k8s.deployment.require_tools",
        lambda tools: (
            calls.append("require_tools"),
            required_tools.append(tuple(tools)),
        ),
    )
    monkeypatch.setattr(
        "local_k8s.deployment.ensure_cluster", record_step("ensure_cluster")
    )
    monkeypatch.setattr(
        "local_k8s.deployment.ensure_namespace", record_step("ensure_namespace")
    )
    monkeypatch.setattr(
        "local_k8s.deployment.ensure_session_secret",
        record_step("ensure_session_secret"),
    )
    monkeypatch.setattr(
        "local_k8s.deployment.import_image", record_step("import_image")
    )
    monkeypatch.setattr(
        "local_k8s.deployment.helm_upgrade", record_step("helm_upgrade")
    )
    monkeypatch.setattr(
        "local_k8s.deployment.print_status", record_step("print_status")
    )
    monkeypatch.setattr("local_k8s.deployment.build_image", record_step("build_image"))

    deploy_preview(preview_config, skip_build=skip_build)

    assert required_tools == [expected_tools], (
        f"expected require_tools to be called once with {expected_tools}, "
        f"but got {required_tools}"
    )
    expected_calls = [
        "require_tools",
        "ensure_cluster",
        "ensure_namespace",
        "ensure_session_secret",
        *([] if skip_build else ["build_image"]),
        "import_image",
        "helm_upgrade",
        "print_status",
    ]
    assert calls == expected_calls, "deploy_preview must preserve lifecycle order"


def test_deploy_preview_tools_follow_configured_kubernetes_provider(
    preview_config: PreviewConfig,
) -> None:
    """Verify provider preflight follows the configured local cluster tool."""
    kind_config = replace(preview_config, k8s_provider="kind")

    assert _deploy_preview_tools(kind_config, skip_build=True) == (
        "helm",
        "kind",
        "kubectl",
    ), "kind preflight must require the kind provider tool alongside helm and kubectl"


def test_deploy_preview_tools_reject_unexpected_kubernetes_provider(
    preview_config: PreviewConfig,
) -> None:
    """Verify provider preflight rejects impossible provider values."""
    invalid_config = replace(preview_config, k8s_provider=cast("K8sProvider", "minikube"))

    with pytest.raises(LocalK8sError, match="Unsupported Kubernetes provider"):
        _deploy_preview_tools(invalid_config, skip_build=True)


def test_build_image_uses_configured_container_engine(
    monkeypatch: pytest.MonkeyPatch,
    preview_config: PreviewConfig,
) -> None:
    """Verify local image builds use Docker or Podman from configuration."""
    # Podman is only supported with the kind provider, so pair the engines.
    podman_config = replace(
        preview_config, container_engine="podman", k8s_provider="kind"
    )
    commands = install_run_recorder(monkeypatch)

    build_image(podman_config)

    assert commands == [
        (
            "podman",
            [
                "build",
                "-f",
                "/repo/deploy/docker/backend.Dockerfile",
                "-t",
                "wildside-backend:local",
                "/repo",
            ],
            None,
        )
    ], "image builds must use the configured container engine"


def test_helm_upgrade_uses_configured_kube_context(
    monkeypatch: pytest.MonkeyPatch,
    preview_config: PreviewConfig,
) -> None:
    """Verify Helm upgrades target the selected provider context."""
    config = replace(preview_config, k8s_provider="kind")
    commands = install_run_recorder(monkeypatch)

    helm_upgrade(config)

    assert commands == [
        (
            "helm",
            [
                "--kube-context",
                "kind-wildside-preview",
                "upgrade",
                "--install",
                "preview",
                "/repo/deploy/charts/wildside",
                "--namespace",
                "wildside",
                "--values",
                "/repo/deploy/charts/wildside/values.local.yaml",
                "--set-string",
                "image.repository=wildside-backend",
                "--set-string",
                "image.tag=local",
                "--wait",
                "--timeout",
                "5m",
            ],
            None,
        )
    ], "Helm upgrades must use the provider-specific kube context"


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


@dataclass(slots=True)
class _ConcurrentSecretResponder:
    """Fake `run` simulating a Secret created concurrently by another process."""

    ready_after_get_calls: int
    commands: list[CommandRecord] = field(default_factory=list)

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
    assert all(
        cmd[1][2:5] != ["apply", "-f", "-"] for cmd in responder.commands
    ), "a valid concurrent Secret must be reused without re-applying it"


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
        ensure_session_secret(preview_config, key_generator=lambda length: b"a" * length)


def _raise_on_create(
    exc: LocalK8sError,
) -> Callable[..., SimpleNamespace]:
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
    conflict = LocalK8sError(
        _already_exists_message(), stderr=_already_exists_stderr()
    )
    monkeypatch.setattr(
        "local_k8s.session_secret.run", _raise_on_create(conflict)
    )
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
    monkeypatch.setattr(
        "local_k8s.session_secret.run", _raise_on_create(failure)
    )
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
    """Verify an incidental "already exists" message without the server reason re-raises."""
    # The message mentions "already exists" but the stderr lacks the structured
    # ``(AlreadyExists)`` server reason, so it must not be treated as a conflict.
    misleading = LocalK8sError(
        'the namespace "already exists" is terminating',
        stderr='Error from server (Forbidden): namespace "already exists" is terminating',
    )
    monkeypatch.setattr(
        "local_k8s.session_secret.run", _raise_on_create(misleading)
    )
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
