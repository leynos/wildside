#!/usr/bin/env -S uv run python
# /// script
# requires-python = ">=3.13"
# dependencies = ["cyclopts>=2.9"]
# ///
"""Render platform manifests via OpenTofu.

This script:
- creates a temporary workspace for the platform_render module;
- runs tofu init and apply to render manifests;
- extracts rendered_manifests output; and
- writes manifests to the output directory.
"""

from __future__ import annotations

import sys
from dataclasses import dataclass
from pathlib import Path

from cyclopts import App, Parameter
from scripts._input_resolution import InputResolution, resolve_input
from scripts._infra_k8s import (
    run_tofu,
    tofu_output,
    write_manifests,
    write_tfvars,
)

REPO_ROOT = Path(__file__).resolve().parents[1]
PLATFORM_RENDER_PATH = REPO_ROOT / "infra" / "modules" / "platform_render"

app = App(help="Render platform manifests via OpenTofu.")


@dataclass(frozen=True, slots=True)
class RenderInputs:
    """Inputs for platform manifest rendering."""

    # Core configuration
    cluster_name: str
    domain: str
    acme_email: str
    cloudflare_api_token_secret_name: str

    # Vault configuration
    vault_address: str | None
    vault_role_id: str | None
    vault_secret_id: str | None
    vault_ca_certificate: str | None

    # Feature flags
    enable_traefik: bool
    enable_cert_manager: bool
    enable_external_dns: bool
    enable_vault_eso: bool
    enable_cnpg: bool

    # Paths
    runner_temp: Path
    output_dir: Path
    github_env: Path


def _parse_bool(value: str | None, default: bool = True) -> bool:
    """Parse a boolean string value."""
    if value is None:
        return default
    return value.lower() in ("true", "1", "yes")


def resolve_render_inputs() -> RenderInputs:
    """Resolve rendering inputs from environment."""
    cluster_name = resolve_input(
        None, InputResolution(env_key="CLUSTER_NAME", required=True)
    )
    domain = resolve_input(None, InputResolution(env_key="DOMAIN", required=True))
    acme_email = resolve_input(
        None, InputResolution(env_key="ACME_EMAIL", required=True)
    )
    cloudflare_api_token_secret_name = resolve_input(
        None,
        InputResolution(
            env_key="CLOUDFLARE_API_TOKEN_SECRET_NAME",
            default="cloudflare-api-token",
        ),
    )

    # Vault configuration
    vault_address = resolve_input(None, InputResolution(env_key="VAULT_ADDRESS"))
    vault_role_id = resolve_input(None, InputResolution(env_key="VAULT_ROLE_ID"))
    vault_secret_id = resolve_input(None, InputResolution(env_key="VAULT_SECRET_ID"))
    vault_ca_certificate = resolve_input(
        None, InputResolution(env_key="VAULT_CA_CERTIFICATE")
    )

    # Feature flags
    enable_traefik_raw = resolve_input(
        None, InputResolution(env_key="ENABLE_TRAEFIK", default="true")
    )
    enable_cert_manager_raw = resolve_input(
        None, InputResolution(env_key="ENABLE_CERT_MANAGER", default="true")
    )
    enable_external_dns_raw = resolve_input(
        None, InputResolution(env_key="ENABLE_EXTERNAL_DNS", default="true")
    )
    enable_vault_eso_raw = resolve_input(
        None, InputResolution(env_key="ENABLE_VAULT_ESO", default="true")
    )
    enable_cnpg_raw = resolve_input(
        None, InputResolution(env_key="ENABLE_CNPG", default="true")
    )

    # Paths
    runner_temp_raw = resolve_input(
        None,
        InputResolution(env_key="RUNNER_TEMP", default=Path("/tmp"), as_path=True),
    )
    output_dir_raw = resolve_input(
        None,
        InputResolution(
            env_key="RENDER_OUTPUT_DIR",
            default=Path("/tmp/rendered-manifests"),
            as_path=True,
        ),
    )
    github_env_raw = resolve_input(
        None,
        InputResolution(
            env_key="GITHUB_ENV",
            default=Path("/tmp/github-env-undefined"),
            as_path=True,
        ),
    )

    return RenderInputs(
        cluster_name=str(cluster_name),
        domain=str(domain),
        acme_email=str(acme_email),
        cloudflare_api_token_secret_name=str(cloudflare_api_token_secret_name),
        vault_address=str(vault_address) if vault_address else None,
        vault_role_id=str(vault_role_id) if vault_role_id else None,
        vault_secret_id=str(vault_secret_id) if vault_secret_id else None,
        vault_ca_certificate=str(vault_ca_certificate)
        if vault_ca_certificate
        else None,
        enable_traefik=_parse_bool(str(enable_traefik_raw)),
        enable_cert_manager=_parse_bool(str(enable_cert_manager_raw)),
        enable_external_dns=_parse_bool(str(enable_external_dns_raw)),
        enable_vault_eso=_parse_bool(str(enable_vault_eso_raw)),
        enable_cnpg=_parse_bool(str(enable_cnpg_raw)),
        runner_temp=(
            runner_temp_raw
            if isinstance(runner_temp_raw, Path)
            else Path(str(runner_temp_raw))
        ),
        output_dir=(
            output_dir_raw
            if isinstance(output_dir_raw, Path)
            else Path(str(output_dir_raw))
        ),
        github_env=(
            github_env_raw
            if isinstance(github_env_raw, Path)
            else Path(str(github_env_raw))
        ),
    )


def build_render_tfvars(inputs: RenderInputs) -> dict[str, object]:
    """Build tfvars for platform rendering."""
    variables: dict[str, object] = {
        "cluster_name": inputs.cluster_name,
        "domain": inputs.domain,
        "acme_email": inputs.acme_email,
        "cloudflare_api_token_secret_name": inputs.cloudflare_api_token_secret_name,
        "traefik_enabled": inputs.enable_traefik,
        "cert_manager_enabled": inputs.enable_cert_manager,
        "external_dns_enabled": inputs.enable_external_dns,
        "vault_eso_enabled": inputs.enable_vault_eso,
        "cnpg_enabled": inputs.enable_cnpg,
    }

    # Vault configuration (only if vault_eso is enabled)
    if inputs.enable_vault_eso:
        if inputs.vault_address:
            variables["vault_address"] = inputs.vault_address
        if inputs.vault_role_id:
            variables["vault_approle_role_id"] = inputs.vault_role_id
        if inputs.vault_secret_id:
            variables["vault_approle_secret_id"] = inputs.vault_secret_id
        if inputs.vault_ca_certificate:
            variables["vault_ca_bundle_pem"] = inputs.vault_ca_certificate

    return variables


def render_manifests(inputs: RenderInputs, tfvars: dict[str, object]) -> dict[str, str]:
    """Run OpenTofu to render platform manifests.

    Returns a dict mapping relative paths to YAML content.
    """
    work_dir = inputs.runner_temp / "render-manifests"
    work_dir.mkdir(parents=True, exist_ok=True)

    # Write tfvars to temp file
    var_file = work_dir / "platform.tfvars.json"
    write_tfvars(var_file, tfvars)

    print(f"Rendering platform manifests for cluster '{inputs.cluster_name}'...")
    print(f"  Domain: {inputs.domain}")
    print(f"  Traefik: {inputs.enable_traefik}")
    print(f"  cert-manager: {inputs.enable_cert_manager}")
    print(f"  external-dns: {inputs.enable_external_dns}")
    print(f"  vault-eso: {inputs.enable_vault_eso}")
    print(f"  CNPG: {inputs.enable_cnpg}")

    # Initialise the module
    print("\n--- Running tofu init ---")
    init_result = run_tofu(["init", "-input=false"], PLATFORM_RENDER_PATH)
    if not init_result.success:
        print(f"error: tofu init failed: {init_result.stderr}", file=sys.stderr)
        raise RuntimeError("tofu init failed")

    print(init_result.stdout)

    # Run apply to render manifests
    print("\n--- Running tofu apply ---")
    apply_result = run_tofu(
        ["apply", "-auto-approve", "-input=false", f"-var-file={var_file}"],
        PLATFORM_RENDER_PATH,
    )
    if not apply_result.success:
        print(f"error: tofu apply failed: {apply_result.stderr}", file=sys.stderr)
        raise RuntimeError("tofu apply failed")

    print(apply_result.stdout)

    # Extract rendered_manifests output
    print("\n--- Extracting rendered manifests ---")
    outputs = tofu_output(PLATFORM_RENDER_PATH)

    rendered_manifests: dict[str, str] = {}
    if "rendered_manifests" in outputs:
        manifests_raw = outputs["rendered_manifests"]
        if isinstance(manifests_raw, dict):
            # Handle nested output structure
            if "value" in manifests_raw:
                manifests_raw = manifests_raw["value"]
            for path, content in manifests_raw.items():
                rendered_manifests[str(path)] = str(content)

    return rendered_manifests


@app.command()
def main(
    cluster_name: str | None = Parameter(),
    domain: str | None = Parameter(),
    acme_email: str | None = Parameter(),
    cloudflare_api_token_secret_name: str | None = Parameter(),
    vault_address: str | None = Parameter(),
    vault_role_id: str | None = Parameter(),
    vault_secret_id: str | None = Parameter(),
    vault_ca_certificate: str | None = Parameter(),
    enable_traefik: str | None = Parameter(),
    enable_cert_manager: str | None = Parameter(),
    enable_external_dns: str | None = Parameter(),
    enable_vault_eso: str | None = Parameter(),
    enable_cnpg: str | None = Parameter(),
    runner_temp: Path | None = Parameter(),
    output_dir: Path | None = Parameter(),
    github_env: Path | None = Parameter(),
) -> int:
    """Render platform manifests via OpenTofu.

    This command resolves inputs from environment variables (set by
    prepare_infra_k8s_inputs.py), runs the platform_render module, and
    writes the rendered manifests to the output directory.
    """
    # Resolve inputs from environment
    inputs = resolve_render_inputs()

    # Build tfvars
    tfvars = build_render_tfvars(inputs)

    try:
        # Render manifests
        manifests = render_manifests(inputs, tfvars)

        if not manifests:
            print("warning: no manifests rendered")
            return 0

        # Write manifests to output directory
        print(f"\n--- Writing {len(manifests)} manifests to {inputs.output_dir} ---")
        count = write_manifests(inputs.output_dir, manifests)

        print(f"\nRendered {count} manifests successfully.")

        # Export count to GITHUB_ENV
        from scripts._infra_k8s import append_github_env

        append_github_env(
            inputs.github_env,
            {
                "RENDERED_MANIFEST_COUNT": str(count),
                "RENDER_OUTPUT_DIR": str(inputs.output_dir),
            },
        )

        return 0

    except RuntimeError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":  # pragma: no cover - CLI entrypoint
    raise SystemExit(app())
