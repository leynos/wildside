#!/usr/bin/env -S uv run python
# /// script
# requires-python = ">=3.13"
# dependencies = ["cyclopts>=2.9"]
# ///
"""Publish outputs for the wildside-infra-k8s GitHub Action.

This script:
- reads computed values from environment variables;
- writes outputs to $GITHUB_OUTPUT; and
- performs a final secret masking pass.
"""

from __future__ import annotations

import os
from dataclasses import dataclass
from pathlib import Path

from cyclopts import App, Parameter
from scripts._input_resolution import InputResolution, resolve_input
from scripts._infra_k8s import append_github_output, mask_secret

app = App(help="Publish wildside-infra-k8s action outputs.")


@dataclass(frozen=True, slots=True)
class OutputValues:
    """Values to publish as action outputs."""

    cluster_name: str | None
    cluster_id: str | None
    cluster_endpoint: str | None
    gitops_commit_sha: str | None
    rendered_manifest_count: str | None


def resolve_output_values() -> OutputValues:
    """Resolve output values from environment."""
    cluster_name = resolve_input(None, InputResolution(env_key="CLUSTER_NAME"))
    cluster_id = resolve_input(None, InputResolution(env_key="CLUSTER_ID"))
    cluster_endpoint = resolve_input(None, InputResolution(env_key="CLUSTER_ENDPOINT"))
    gitops_commit_sha = resolve_input(
        None, InputResolution(env_key="GITOPS_COMMIT_SHA")
    )
    rendered_manifest_count = resolve_input(
        None, InputResolution(env_key="RENDERED_MANIFEST_COUNT")
    )

    return OutputValues(
        cluster_name=str(cluster_name) if cluster_name else None,
        cluster_id=str(cluster_id) if cluster_id else None,
        cluster_endpoint=str(cluster_endpoint) if cluster_endpoint else None,
        gitops_commit_sha=str(gitops_commit_sha) if gitops_commit_sha else None,
        rendered_manifest_count=(
            str(rendered_manifest_count) if rendered_manifest_count else None
        ),
    )


def publish_outputs(values: OutputValues, github_output: Path) -> None:
    """Write outputs to GITHUB_OUTPUT file."""
    outputs: dict[str, str] = {}

    if values.cluster_name:
        outputs["cluster_name"] = values.cluster_name

    if values.cluster_id:
        outputs["cluster_id"] = values.cluster_id

    if values.cluster_endpoint:
        outputs["cluster_endpoint"] = values.cluster_endpoint

    if values.gitops_commit_sha:
        outputs["gitops_commit_sha"] = values.gitops_commit_sha

    if values.rendered_manifest_count:
        outputs["rendered_manifest_count"] = values.rendered_manifest_count

    if outputs:
        append_github_output(github_output, outputs)
        print(f"Published {len(outputs)} outputs to GITHUB_OUTPUT")
        for key, value in outputs.items():
            print(f"  {key}: {value}")


def final_secret_masking() -> None:
    """Perform final pass to ensure sensitive values are masked."""
    # List of environment variables that should be masked if present
    sensitive_keys = [
        "GITOPS_TOKEN",
        "VAULT_ROLE_ID",
        "VAULT_SECRET_ID",
        "DIGITALOCEAN_TOKEN",
        "SPACES_ACCESS_KEY",
        "SPACES_SECRET_KEY",
        "KUBECONFIG_RAW",
        "VAULT_CA_CERTIFICATE",
    ]

    for key in sensitive_keys:
        value = os.environ.get(key)
        if value:
            mask_secret(value)


@app.command()
def main(
    cluster_name: str | None = Parameter(),
    cluster_id: str | None = Parameter(),
    cluster_endpoint: str | None = Parameter(),
    gitops_commit_sha: str | None = Parameter(),
    rendered_manifest_count: str | None = Parameter(),
    github_output: Path | None = Parameter(),
) -> int:
    """Publish outputs for the wildside-infra-k8s action.

    This command reads computed values from environment variables and
    writes them to GITHUB_OUTPUT for use by subsequent workflow steps.
    """
    # Resolve GITHUB_OUTPUT path
    github_output_raw = resolve_input(
        github_output,
        InputResolution(
            env_key="GITHUB_OUTPUT",
            default=Path("/tmp/github-output-undefined"),
            as_path=True,
        ),
    )
    github_output_path = (
        github_output_raw
        if isinstance(github_output_raw, Path)
        else Path(str(github_output_raw))
    )

    # Resolve output values from environment
    values = resolve_output_values()

    # Perform final secret masking
    final_secret_masking()

    # Publish outputs
    publish_outputs(values, github_output_path)

    print("\nOutput publishing complete.")
    return 0


if __name__ == "__main__":  # pragma: no cover - CLI entrypoint
    raise SystemExit(app())
