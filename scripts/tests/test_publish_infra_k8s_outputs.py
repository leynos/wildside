"""Unit tests for publish_infra_k8s_outputs."""

from __future__ import annotations

import sys
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from scripts.publish_infra_k8s_outputs import (  # noqa: E402
    OutputValues,
    final_secret_masking,
    publish_outputs,
    resolve_output_values,
)


def test_resolve_output_values(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("CLUSTER_NAME", "preview")
    monkeypatch.setenv("CLUSTER_ID", "abc")
    monkeypatch.setenv("CLUSTER_ENDPOINT", "https://kube")

    values = resolve_output_values()
    assert values.cluster_name == "preview"
    assert values.cluster_id == "abc"
    assert values.cluster_endpoint == "https://kube"


def test_publish_outputs_writes_file(tmp_path: Path) -> None:
    output_file = tmp_path / "out"
    values = OutputValues(
        cluster_name="preview",
        cluster_id="abc",
        cluster_endpoint=None,
        gitops_commit_sha="sha",
        rendered_manifest_count="3",
    )

    publish_outputs(values, output_file)

    content = output_file.read_text(encoding="utf-8")
    assert "cluster_name=preview" in content
    assert "gitops_commit_sha=sha" in content
    assert "rendered_manifest_count=3" in content


def test_final_secret_masking(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("GITOPS_TOKEN", "token")
    masked: list[str] = []
    monkeypatch.setattr("scripts.publish_infra_k8s_outputs.mask_secret", masked.append)

    final_secret_masking()

    assert "token" in masked
