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
    append_github_env,
    parse_bool,
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


def resolve_render_inputs(
    *,
    cluster_name: str | None = None,
    domain: str | None = None,
    acme_email: str | None = None,
    cloudflare_api_token_secret_name: str | None = None,
    vault_address: str | None = None,
    vault_role_id: str | None = None,
    vault_secret_id: str | None = None,
    vault_ca_certificate: str | None = None,
    enable_traefik: str | None = None,
    enable_cert_manager: str | None = None,
    enable_external_dns: str | None = None,
    enable_vault_eso: str | None = None,
    enable_cnpg: str | None = None,
    runner_temp: Path | None = None,
    output_dir: Path | None = None,
    github_env: Path | None = None,
) -> RenderInputs:
    """Resolve rendering inputs from environment."""
    cluster_name_raw = resolve_input(
        cluster_name, InputResolution(env_key="CLUSTER_NAME", required=True)
    )
    domain_raw = resolve_input(
        domain, InputResolution(env_key="DOMAIN", required=True)
    )
    acme_email_raw = resolve_input(
        acme_email, InputResolution(env_key="ACME_EMAIL", required=True)
    )
    cloudflare_secret_raw = resolve_input(
        cloudflare_api_token_secret_name,
        InputResolution(
            env_key="CLOUDFLARE_API_TOKEN_SECRET_NAME",
            default="cloudflare-api-token",
        ),
    )

    vault_address_raw = resolve_input(
        vault_address, InputResolution(env_key="VAULT_ADDRESS")
    )
    vault_role_id_raw = resolve_input(
        vault_role_id, InputResolution(env_key="VAULT_ROLE_ID")
    )
    vault_secret_id_raw = resolve_input(
        vault_secret_id, InputResolution(env_key="VAULT_SECRET_ID")
    )
    vault_ca_cert_raw = resolve_input(
        vault_ca_certificate, InputResolution(env_key="VAULT_CA_CERTIFICATE")
    )

    traefik_raw = resolve_input(
        enable_traefik, InputResolution(env_key="ENABLE_TRAEFIK", default="true")
    )
    cert_manager_raw = resolve_input(
        enable_cert_manager, InputResolution(env_key="ENABLE_CERT_MANAGER", default="true")
    )
    external_dns_raw = resolve_input(
        enable_external_dns,
        InputResolution(env_key="ENABLE_EXTERNAL_DNS", default="true"),
    )
    vault_eso_raw = resolve_input(
        enable_vault_eso, InputResolution(env_key="ENABLE_VAULT_ESO", default="true")
    )
    cnpg_raw = resolve_input(
        enable_cnpg, InputResolution(env_key="ENABLE_CNPG", default="true")
    )

    runner_temp_raw = resolve_input(
        runner_temp,
        InputResolution(env_key="RUNNER_TEMP", default=Path("/tmp"), as_path=True),
    )
    output_dir_raw = resolve_input(
        output_dir,
        InputResolution(
            env_key="RENDER_OUTPUT_DIR",
            default=Path("/tmp/rendered-manifests"),
            as_path=True,
        ),
    )
    github_env_raw = resolve_input(
        github_env,
        InputResolution(
            env_key="GITHUB_ENV",
            default=Path("/tmp/github-env-undefined"),
            as_path=True,
        ),
    )

    return RenderInputs(
        cluster_name=str(cluster_name_raw),
        domain=str(domain_raw),
        acme_email=str(acme_email_raw),
        cloudflare_api_token_secret_name=str(cloudflare_secret_raw),
        vault_address=str(vault_address_raw) if vault_address_raw else None,
        vault_role_id=str(vault_role_id_raw) if vault_role_id_raw else None,
        vault_secret_id=str(vault_secret_id_raw) if vault_secret_id_raw else None,
        vault_ca_certificate=str(vault_ca_cert_raw) if vault_ca_cert_raw else None,
        enable_traefik=parse_bool(str(traefik_raw)),
        enable_cert_manager=parse_bool(str(cert_manager_raw)),
        enable_external_dns=parse_bool(str(external_dns_raw)),
        enable_vault_eso=parse_bool(str(vault_eso_raw)),
        enable_cnpg=parse_bool(str(cnpg_raw)),
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
            github_env_raw if isinstance(github_env_raw, Path) else Path(str(github_env_raw))
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


def _run_tofu_or_raise(command_name: str, args: list[str], module_path: Path) -> None:
    """Run an OpenTofu command and raise on failure.

    Parameters
    ----------
    command_name : str
        Human-readable name for error messages (e.g., "init", "apply").
    args : list[str]
        Command arguments to pass to run_tofu.
    module_path : Path
        Working directory for the command.
    """
    print(f"\n--- Running tofu {command_name} ---")
    result = run_tofu(args, module_path)
    if not result.success:
        print(f"error: tofu {command_name} failed: {result.stderr}", file=sys.stderr)
        raise RuntimeError(f"tofu {command_name} failed")
    print(result.stdout)


def _extract_rendered_manifests(outputs: dict[str, object]) -> dict[str, str]:
    """Extract rendered manifests from OpenTofu outputs.

    Handles both direct output format and wrapped format where the value
    is nested under a "value" key.
    """
    if "rendered_manifests" not in outputs:
        return {}
    manifests_raw = outputs["rendered_manifests"]
    if not isinstance(manifests_raw, dict):
        return {}
    # Handle nested output structure
    if "value" in manifests_raw:
        manifests_raw = manifests_raw["value"]
    return {str(path): str(content) for path, content in manifests_raw.items()}


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

    _run_tofu_or_raise("init", ["init", "-input=false"], PLATFORM_RENDER_PATH)
    _run_tofu_or_raise(
        "apply",
        ["apply", "-auto-approve", "-input=false", f"-var-file={var_file}"],
        PLATFORM_RENDER_PATH,
    )

    print("\n--- Extracting rendered manifests ---")
    outputs = tofu_output(PLATFORM_RENDER_PATH)
    return _extract_rendered_manifests(outputs)


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
    inputs = resolve_render_inputs(
        cluster_name=cluster_name,
        domain=domain,
        acme_email=acme_email,
        cloudflare_api_token_secret_name=cloudflare_api_token_secret_name,
        vault_address=vault_address,
        vault_role_id=vault_role_id,
        vault_secret_id=vault_secret_id,
        vault_ca_certificate=vault_ca_certificate,
        enable_traefik=enable_traefik,
        enable_cert_manager=enable_cert_manager,
        enable_external_dns=enable_external_dns,
        enable_vault_eso=enable_vault_eso,
        enable_cnpg=enable_cnpg,
        runner_temp=runner_temp,
        output_dir=output_dir,
        github_env=github_env,
    )

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
