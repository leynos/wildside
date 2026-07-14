"""Property tests for local Kubernetes preview helper invariants."""

from __future__ import annotations

import os
from dataclasses import replace
from pathlib import Path

import pytest

hypothesis = pytest.importorskip("hypothesis")
st = pytest.importorskip("hypothesis.strategies")

from hypothesis import HealthCheck, given, settings
from hypothesis.strategies import DrawFn

from local_k8s.cluster import import_image
from local_k8s.config import PreviewConfig
from local_k8s.k8s import print_kubernetes_status

_NAME_CHARS = tuple("abcdefghijklmnopqrstuvwxyz0123456789")
_NAME_REST_CHARS = tuple("abcdefghijklmnopqrstuvwxyz0123456789-")
_TAG_CHARS = tuple("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_.-")


@st.composite
def cluster_names(draw: DrawFn) -> str:
    """Generate valid local preview cluster names without filtering."""
    first = draw(st.sampled_from(_NAME_CHARS))
    middle = draw(st.lists(st.sampled_from(_NAME_REST_CHARS), max_size=61)).copy()
    if middle and middle[-1] == "-":
        middle[-1] = "0"
    return first + "".join(middle)


@st.composite
def helm_names(draw: DrawFn) -> str:
    """Generate Helm release and chart names, including truncation cases."""
    first = draw(st.sampled_from(_NAME_CHARS))
    middle = draw(st.lists(st.sampled_from(_NAME_REST_CHARS), max_size=90)).copy()
    if middle and middle[-1] == "-":
        middle[-1] = "0"
    return first + "".join(middle)


@st.composite
def repository_components(draw: DrawFn) -> str:
    """Generate image repository path components that are not registry hosts."""
    component = draw(cluster_names())
    if component == "localhost":
        return "local-host"
    return component


@st.composite
def image_tags(draw: DrawFn) -> str:
    """Generate non-empty Docker image tags."""
    first = draw(st.sampled_from(_TAG_CHARS))
    rest = draw(st.lists(st.sampled_from(_TAG_CHARS), max_size=20))
    return first + "".join(rest)


@st.composite
def registry_qualified_image_names(draw: DrawFn) -> str:
    """Generate registry-qualified image names that must remain unchanged."""
    registry = draw(st.sampled_from(("localhost", "registry.example.test", "localhost:5000")))
    namespace = draw(repository_components())
    repository = draw(repository_components())
    tag = draw(image_tags())
    return f"{registry}/{namespace}/{repository}:{tag}"


def _preview_config(*, release_name: str, chart_name: str) -> PreviewConfig:
    """Return a minimal PreviewConfig for pure helper property tests."""
    chart_path = Path("/repo/deploy/charts") / chart_name
    return PreviewConfig(
        repository_root=Path("/repo"),
        container_engine="docker",
        k8s_provider="k3d",
        cluster_name="wildside-preview",
        namespace="wildside",
        release_name=release_name,
        image_name="wildside-backend:local",
        kind_node_image="kindest/node:v1.31.0",
        ingress_port=8088,
        chart_path=chart_path,
        local_values_path=chart_path / "values.local.yaml",
        dockerfile_path=Path("/repo/deploy/docker/backend.Dockerfile"),
    )


def _podman_saved_image_name(
    monkeypatch: pytest.MonkeyPatch, image_name: str, tmp_path: Path
) -> str:
    """Return the image name passed to ``podman save`` for a preview import."""
    commands: list[tuple[str, list[str]]] = []

    def record_run(command: str, args: list[str], **_: object) -> object:
        commands.append((command, args))
        return object()

    config = _preview_config(release_name="wildside", chart_name="wildside")
    monkeypatch.setattr("local_k8s.cluster.require_tools", lambda _: None)
    monkeypatch.setattr("local_k8s.cluster.run", record_run)
    monkeypatch.setattr("local_k8s.cluster._remove_stale_archive", lambda _: None)
    # This property exercises only the archived image name, so stub the archive
    # path helper to avoid touching the filesystem for each generated example.
    monkeypatch.setattr(
        "local_k8s.cluster._image_archive_path",
        lambda _config, *, archive_dir=None: Path(archive_dir or tmp_path)
        / "wildside-preview-image.tar",
    )

    import_image(
        replace(
            config,
            container_engine="podman",
            k8s_provider="kind",
            image_name=image_name,
        ),
    )

    save_commands = [args for command, args in commands if command == "podman" and args[0] == "save"]
    assert len(save_commands) == 1, "Podman import must issue exactly one save command"
    return save_commands[0][-1]


def _printed_service_name(monkeypatch: pytest.MonkeyPatch, config: PreviewConfig) -> str:
    """Return the service name requested by ``print_kubernetes_status``."""
    commands: list[tuple[str, list[str]]] = []

    def record_run(command: str, args: list[str], **_: object) -> object:
        commands.append((command, args))
        return type("Result", (), {"stdout": ""})()

    monkeypatch.setattr("local_k8s.k8s.require_tools", lambda _: None)
    monkeypatch.setattr("local_k8s.k8s.run", record_run)

    print_kubernetes_status(config)

    service_commands = [
        args
        for command, args in commands
        if command == "kubectl" and "service" in args and "--ignore-not-found" in args
    ]
    assert len(service_commands) == 1, (
        "status must query exactly one Kubernetes service"
    )
    return service_commands[0][-2]


@settings(suppress_health_check=[HealthCheck.function_scoped_fixture])
@given(repository=repository_components(), tag=image_tags())
def test_podman_archive_image_name_prefixes_unqualified_images(
    monkeypatch: pytest.MonkeyPatch,
    repository: str,
    tag: str,
    tmp_path: Path,
) -> None:
    """Verify unqualified images normalize to Docker Hub library pulls."""
    image_name = f"{repository}:{tag}"

    assert _podman_saved_image_name(monkeypatch, image_name, tmp_path) == (
        f"docker.io/library/{image_name}"
    ), "unqualified images must normalize to the Docker Hub library namespace"


@settings(suppress_health_check=[HealthCheck.function_scoped_fixture])
@given(namespace=repository_components(), repository=repository_components(), tag=image_tags())
def test_podman_archive_image_name_prefixes_namespaced_images(
    monkeypatch: pytest.MonkeyPatch,
    namespace: str,
    repository: str,
    tag: str,
    tmp_path: Path,
) -> None:
    """Verify namespaced Docker Hub images keep their namespace."""
    image_name = f"{namespace}/{repository}:{tag}"

    assert _podman_saved_image_name(monkeypatch, image_name, tmp_path) == (
        f"docker.io/{image_name}"
    ), "namespaced Docker Hub images must keep their namespace under docker.io"


@settings(suppress_health_check=[HealthCheck.function_scoped_fixture])
@given(image_name=registry_qualified_image_names())
def test_podman_archive_image_name_preserves_registry_hosts(
    monkeypatch: pytest.MonkeyPatch,
    image_name: str,
    tmp_path: Path,
) -> None:
    """Verify registry-qualified images are already Kubernetes-compatible."""
    assert _podman_saved_image_name(monkeypatch, image_name, tmp_path) == image_name, (
        "registry-qualified images must be archived without rewriting"
    )


@settings(suppress_health_check=[HealthCheck.function_scoped_fixture])
@given(release_name=helm_names(), chart_name=helm_names())
def test_helm_fullname_obeys_kubernetes_name_bounds(
    monkeypatch: pytest.MonkeyPatch,
    release_name: str,
    chart_name: str,
) -> None:
    """Verify Helm fullnames stay inside Kubernetes DNS label bounds."""
    fullname = _printed_service_name(
        monkeypatch,
        _preview_config(release_name=release_name, chart_name=chart_name),
    )

    assert len(fullname) <= 63, "Helm fullname must fit within the 63-char DNS label limit"
    assert not fullname.endswith("-"), "Helm fullname must not end with a trailing hyphen"
    if release_name == chart_name:
        assert fullname == chart_name, (
            "a matching release and chart name must collapse to the chart name"
        )
    else:
        assert fullname == f"{release_name}-{chart_name}"[:63].rstrip("-"), (
            "Helm fullname must be the truncated release-chart join"
        )


@settings(suppress_health_check=[HealthCheck.function_scoped_fixture])
@given(
    cluster_name=cluster_names(),
    legacy_cluster_name=cluster_names(),
    ingress_port=st.integers(min_value=1, max_value=65535),
    legacy_ingress_port=st.integers(min_value=1, max_value=65535),
)
def test_provider_neutral_env_overrides_legacy_aliases(
    cluster_name: str,
    legacy_cluster_name: str,
    ingress_port: int,
    legacy_ingress_port: int,
) -> None:
    """Verify provider-neutral environment names take precedence."""
    with pytest.MonkeyPatch.context() as monkeypatch:
        for name in tuple(os.environ):
            if name.startswith("WILDSIDE_"):
                monkeypatch.delenv(name, raising=False)
        monkeypatch.setenv("WILDSIDE_K8S_CLUSTER", cluster_name)
        monkeypatch.setenv("WILDSIDE_K3D_CLUSTER", legacy_cluster_name)
        monkeypatch.setenv("WILDSIDE_K8S_PORT", str(ingress_port))
        monkeypatch.setenv("WILDSIDE_K3D_PORT", str(legacy_ingress_port))

        config = PreviewConfig.from_env()

    assert config.cluster_name == cluster_name, (
        "provider-neutral WILDSIDE_K8S_CLUSTER must override the legacy K3D alias"
    )
    assert config.ingress_port == ingress_port, (
        "provider-neutral WILDSIDE_K8S_PORT must override the legacy K3D alias"
    )
