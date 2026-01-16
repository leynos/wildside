# Output values for the platform_render orchestration module.
#
# The primary output is `rendered_manifests`, a map of GitOps paths to YAML
# content. This map can be iterated to write files to the GitOps repository.

output "rendered_manifests" {
  description = "Merged Flux-ready manifests keyed by GitOps path"
  value       = local.all_manifests
  sensitive   = true
}

output "manifest_count" {
  description = "Total number of rendered manifests"
  value       = local.actual_total
}

output "manifest_counts_by_module" {
  description = "Manifest counts grouped by module"
  value       = local.manifest_counts
}

output "enabled_modules" {
  description = "List of enabled platform modules"
  value = compact([
    var.traefik_enabled ? "traefik" : "",
    var.cert_manager_enabled ? "cert_manager" : "",
    var.external_dns_enabled ? "external_dns" : "",
    var.vault_eso_enabled ? "vault_eso" : "",
    var.cnpg_enabled ? "cnpg" : "",
  ])
}

# -----------------------------------------------------------------------------
# Module-specific outputs (for downstream consumers)
# -----------------------------------------------------------------------------

output "traefik_ingress_class_name" {
  description = "Name of the Traefik IngressClass (null if Traefik is disabled)"
  value       = var.traefik_enabled ? module.traefik[0].ingress_class_name : null
}

output "traefik_cluster_issuer_name" {
  description = "Name of the Traefik ClusterIssuer (null if Traefik is disabled)"
  value       = var.traefik_enabled ? module.traefik[0].cluster_issuer_name : null
}

output "cert_manager_vault_issuer_name" {
  description = "Name of the Vault ClusterIssuer (null if cert-manager or Vault is disabled)"
  value = (
    var.cert_manager_enabled && var.cert_manager_vault_enabled
    ? module.cert_manager[0].vault_issuer_name
    : null
  )
}

output "external_dns_txt_owner_id" {
  description = "TXT owner ID for ExternalDNS (null if ExternalDNS is disabled)"
  value       = var.external_dns_enabled ? module.external_dns[0].txt_owner_id : null
}

output "vault_eso_cluster_secret_store_name" {
  description = "Name of the Vault KV ClusterSecretStore (null if Vault ESO is disabled)"
  value       = var.vault_eso_enabled ? module.vault_eso[0].cluster_secret_store_kv_name : null
}

output "cnpg_primary_endpoint" {
  description = "Primary endpoint for CNPG database connections (null if CNPG is disabled)"
  value       = var.cnpg_enabled ? module.cnpg[0].primary_endpoint : null
}

output "cnpg_replica_endpoint" {
  description = "Replica endpoint for CNPG read-only connections (null if CNPG is disabled)"
  value       = var.cnpg_enabled ? module.cnpg[0].replica_endpoint : null
}

# Note: Valkey outputs are temporarily excluded due to provider version
# incompatibility. See main.tf comments for details.

# -----------------------------------------------------------------------------
# Sync Policy Contracts (for workload integration)
# -----------------------------------------------------------------------------

output "vault_eso_sync_policy_contract" {
  description = "Vault ESO sync policy contract for downstream workloads"
  value       = var.vault_eso_enabled ? module.vault_eso[0].sync_policy_contract : null
}

output "cnpg_sync_policy_contract" {
  description = "CNPG sync policy contract for downstream workloads"
  value       = var.cnpg_enabled ? module.cnpg[0].sync_policy_contract : null
}
