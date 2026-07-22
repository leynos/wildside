"""Helm render smoke checks for the local preview chart values.

These render the wildside chart with ``values.local.yaml`` and assert the
session-secret mount path and session-key environment survive templating, so a
values regression that breaks the local preview is caught before deploy.
"""

from __future__ import annotations

import shutil
import subprocess  # noqa: S404 - test drives helm via subprocess.
from pathlib import Path

import pytest
import yaml

REPO_ROOT = Path(__file__).resolve().parents[3]
CHART_DIR = REPO_ROOT / "deploy" / "charts" / "wildside"
LOCAL_VALUES = CHART_DIR / "values.local.yaml"
SESSION_MOUNT_PATH = "/var/run/secrets/wildside-session"
HELM_TEMPLATE_TIMEOUT_SECONDS = 120

type YamlScalar = str | int | float | bool | None
type YamlValue = YamlScalar | list[YamlValue] | dict[str, YamlValue]
type Manifest = dict[str, YamlValue]


def _manifests(rendered: str) -> list[Manifest]:
    """Return the rendered multi-document YAML as a list of manifest mappings."""
    return [doc for doc in yaml.safe_load_all(rendered) if isinstance(doc, dict)]


def _first_of_kind(manifests: list[Manifest], kind: str) -> Manifest:
    """Return the first rendered manifest with the given ``kind``."""
    return next(manifest for manifest in manifests if manifest.get("kind") == kind)


def _dig(mapping: dict[str, YamlValue], *keys: str) -> YamlValue:
    """Walk nested YAML mappings by ``keys``, asserting each level is a mapping."""
    value: YamlValue = mapping
    for key in keys:
        assert isinstance(value, dict), (
            f"expected a mapping while walking to {key!r} via {keys!r}, "
            f"got {type(value).__name__}"
        )
        value = value[key]
    return value


@pytest.fixture(scope="module")
def local_preview_render() -> str:
    """Render the wildside chart with the local preview values."""
    helm = shutil.which("helm")
    if helm is None:
        pytest.skip("helm is required to render the local preview chart")
    completed = subprocess.run(  # noqa: S603 - argv is fixed by the test.
        [
            helm,
            "template",
            "wildside",
            str(CHART_DIR),
            "--values",
            str(LOCAL_VALUES),
            "--kube-version",
            "1.31.0",
        ],
        capture_output=True,
        text=True,
        check=True,
        timeout=HELM_TEMPLATE_TIMEOUT_SECONDS,
    )
    return completed.stdout


def test_local_render_mounts_session_secret_path(local_preview_render: str) -> None:
    """Verify the rendered manifests mount the local session secret path."""
    assert f"mountPath: {SESSION_MOUNT_PATH}" in local_preview_render, (
        "local preview render must mount the session secret at the configured path"
    )


def test_local_render_wires_session_key_env(local_preview_render: str) -> None:
    """Verify the backend resolves SESSION_KEY_FILE to the mounted session key."""
    manifests = _manifests(local_preview_render)
    deployment = _first_of_kind(manifests, "Deployment")
    config_map = _first_of_kind(manifests, "ConfigMap")

    containers = _dig(deployment, "spec", "template", "spec", "containers")
    assert isinstance(containers, list), (
        "Deployment spec.template.spec.containers must be a list"
    )
    env_by_name: dict[str, Manifest] = {}
    for container in containers:
        assert isinstance(container, dict), (
            "each Deployment container must be a mapping"
        )
        env = container.get("env", [])
        assert isinstance(env, list), "container 'env' must be a list of env entries"
        for entry in env:
            assert isinstance(entry, dict), "each container env entry must be a mapping"
            name = entry["name"]
            assert isinstance(name, str), (
                "each container env entry 'name' must be a string"
            )
            env_by_name[name] = entry
    session_env = env_by_name["SESSION_KEY_FILE"]
    config_map_key = _dig(session_env, "valueFrom", "configMapKeyRef", "key")
    assert isinstance(config_map_key, str), (
        "SESSION_KEY_FILE valueFrom.configMapKeyRef.key must be a string"
    )

    data = _dig(config_map, "data")
    assert isinstance(data, dict), "ConfigMap 'data' must be a mapping"
    assert data[config_map_key] == f"{SESSION_MOUNT_PATH}/session_key", (
        "SESSION_KEY_FILE must resolve to the mounted session key path"
    )
