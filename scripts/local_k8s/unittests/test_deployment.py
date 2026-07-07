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
from dataclasses import replace
from pathlib import Path
from types import SimpleNamespace

import pytest

from local_k8s.config import PreviewConfig
from local_k8s.deployment import (
    _deploy_preview_tools,
    build_image,
    deploy_preview,
    ensure_session_secret,
    helm_upgrade,
    print_logs,
    print_status,
)
from local_k8s.validation import LocalK8sError

CommandRecord = tuple[str, list[str], str | None]
RunHook = Callable[[str, list[str], str | None], None]


@pytest.fixture
def preview_config() -> PreviewConfig:
    """Representative local preview configuration for deployment tests.

    Returns
    -------
    PreviewConfig
        Configuration for a local preview release named ``preview`` in the
        ``wildside`` namespace. The image tag, chart path, values path, and
        Dockerfile path match the deployment fields that ``deploy_preview``
        passes through its build, import, and Helm orchestration steps.
    """

    return PreviewConfig(
        repository_root=Path("/repo"),
        container_engine="docker",
        k8s_provider="k3d",
        cluster_name="wildside-preview",
        namespace="wildside",
        release_name="preview",
        image_name="wildside-backend:local",
        kind_node_image="kindest/node:v1.31.0",
        ingress_port=8088,
        chart_path=Path("/repo/deploy/charts/wildside"),
        local_values_path=Path("/repo/deploy/charts/wildside/values.local.yaml"),
        dockerfile_path=Path("/repo/deploy/docker/backend.Dockerfile"),
    )


def install_run_recorder(
    monkeypatch: pytest.MonkeyPatch,
    *,
    stdout: str = "",
    on_run: RunHook | None = None,
) -> list[CommandRecord]:
    """Replace deployment command execution with a command recorder.

    Parameters
    ----------
    monkeypatch : pytest.MonkeyPatch
        Pytest monkeypatch fixture used to replace ``local_k8s.deployment.run``.
    stdout : str, optional
        Standard output returned by every recorded command.
    on_run : RunHook | None, optional
        Callback invoked with the command, argument list, and optional input
        text before the command is recorded.

    Returns
    -------
    list[CommandRecord]
        Mutable command log populated with ``(command, args, input_text)``
        records for each deployment command invocation.
    """
    commands: list[CommandRecord] = []

    def record_run(command: str, args: list[str], **kwargs: object) -> SimpleNamespace:
        input_text = kwargs.get("input_text")
        if input_text is not None and not isinstance(input_text, str):
            raise AssertionError("input_text must be text when provided")
        if on_run is not None:
            on_run(command, args, input_text)
        commands.append((command, args, input_text))
        return SimpleNamespace(stdout=stdout)

    monkeypatch.setattr("local_k8s.deployment.run", record_run)
    return commands


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
        lambda tools: (calls.append("require_tools"), required_tools.append(tuple(tools))),
    )
    monkeypatch.setattr("local_k8s.deployment.ensure_cluster", record_step("ensure_cluster"))
    monkeypatch.setattr("local_k8s.deployment.ensure_namespace", record_step("ensure_namespace"))
    monkeypatch.setattr("local_k8s.deployment.ensure_session_secret", record_step("ensure_session_secret"))
    monkeypatch.setattr("local_k8s.deployment.import_image", record_step("import_image"))
    monkeypatch.setattr("local_k8s.deployment.helm_upgrade", record_step("helm_upgrade"))
    monkeypatch.setattr("local_k8s.deployment.print_status", record_step("print_status"))
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
    )


def test_build_image_uses_configured_container_engine(
    monkeypatch: pytest.MonkeyPatch,
    preview_config: PreviewConfig,
) -> None:
    """Verify local image builds use Docker or Podman from configuration."""
    podman_config = replace(preview_config, container_engine="podman")
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
                "--set",
                "image.repository=wildside-backend",
                "--set",
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
    assert commands[1] == (
        "kubectl",
        [
            "--context",
            "k3d-wildside-preview",
            "create",
            "-f",
            "-",
        ],
        commands[1][2],
    ), "local preview must atomically create the session Secret before Helm"
    manifest = commands[1][2]
    assert manifest is not None
    assert "name: wildside-session-key" in manifest
    assert "namespace: wildside" in manifest
    encoded_key = manifest.rsplit("session_key: ", maxsplit=1)[1].strip()
    assert base64.b64decode(encoded_key) == b"a" * 96


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
        raise AssertionError("existing local preview session keys must be reused")

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


def test_ensure_session_secret_reuses_concurrent_create(
    monkeypatch: pytest.MonkeyPatch,
    preview_config: PreviewConfig,
) -> None:
    """Verify duplicate deploys do not overwrite a concurrently created Secret."""
    commands: list[CommandRecord] = []

    def record_run(command: str, args: list[str], **kwargs: object) -> SimpleNamespace:
        input_text = kwargs.get("input_text")
        if input_text is not None and not isinstance(input_text, str):
            raise AssertionError("input_text must be text when provided")
        commands.append((command, args, input_text))
        if args[4:7] == ["get", "secret", "wildside-session-key"]:
            return SimpleNamespace(stdout="")
        if args[2:5] == ["create", "-f", "-"]:
            raise LocalK8sError('secrets "wildside-session-key" already exists')
        raise AssertionError(f"unexpected command: {command} {args}")

    monkeypatch.setattr("local_k8s.deployment.run", record_run)

    ensure_session_secret(preview_config, key_generator=lambda length: b"a" * length)

    assert len(commands) == 2, "concurrent create reuse must stop after the create conflict"


def test_print_status_uses_provider_context_and_prints_kind_port_forward(
    monkeypatch: pytest.MonkeyPatch,
    capsys: pytest.CaptureFixture[str],
    preview_config: PreviewConfig,
) -> None:
    """Verify kind status uses provider tools and prints the operator command."""
    config = replace(preview_config, k8s_provider="kind")
    required_tools: list[tuple[str, ...]] = []
    calls: list[str] = []
    commands = install_run_recorder(
        monkeypatch,
        stdout="helm status\n",
        on_run=lambda _command, _args, _input_text: calls.append("helm_status"),
    )

    monkeypatch.setattr(
        "local_k8s.deployment.require_tools",
        lambda tools: (calls.append("require_tools"), required_tools.append(tuple(tools))),
    )
    monkeypatch.setattr(
        "local_k8s.deployment.print_cluster_status",
        lambda _: calls.append("print_cluster_status"),
    )
    monkeypatch.setattr(
        "local_k8s.deployment.print_kubernetes_status",
        lambda _: calls.append("print_kubernetes_status"),
    )

    print_status(config)

    output = capsys.readouterr().out
    assert required_tools == [("helm", "kind", "kubectl")]
    assert calls == [
        "require_tools",
        "print_cluster_status",
        "helm_status",
        "print_kubernetes_status",
    ]
    assert commands == [
        (
            "helm",
            [
                "--kube-context",
                "kind-wildside-preview",
                "-n",
                "wildside",
                "status",
                "preview",
            ],
            None,
        )
    ], "Helm status must use the provider-specific kube context"
    assert (
        "kubectl --context kind-wildside-preview --namespace wildside "
        "port-forward svc/preview-wildside 8088:80"
    ) in output, "kind status must print the port-forward command for the Helm service"


def test_print_logs_uses_configured_kube_context(
    monkeypatch: pytest.MonkeyPatch,
    preview_config: PreviewConfig,
) -> None:
    """Verify log streaming targets the provider-specific kube context."""
    config = replace(preview_config, k8s_provider="kind")
    commands = install_run_recorder(monkeypatch)
    streaming_commands: list[tuple[str, list[str]]] = []

    def record_streaming(command: str, args: list[str]) -> None:
        streaming_commands.append((command, args))

    monkeypatch.setattr("local_k8s.deployment.require_tools", lambda _: None)
    monkeypatch.setattr("local_k8s.deployment.run_streaming", record_streaming)

    print_logs(config, follow=True)

    assert commands == [], "followed logs must stream rather than capture output"
    assert streaming_commands == [
        (
            "kubectl",
            [
                "--context",
                "kind-wildside-preview",
                "-n",
                "wildside",
                "logs",
                "-l",
                "app.kubernetes.io/instance=preview",
                "-c",
                "app",
                "--tail",
                "200",
                "--follow",
            ],
        )
    ], "logs must use the provider-specific kube context"
