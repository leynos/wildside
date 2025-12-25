output "namespace" {
  description = "Namespace where External Secrets Operator is installed"
  value       = local.effective_namespace
}

output "helm_release_name" {
  description = "Name of the External Secrets Operator Helm release"
  value       = local.helm_release_name
}

output "cluster_secret_store_kv_name" {
  description = "Name of the ClusterSecretStore for Vault KV v2 engine"
  value       = local.cluster_secret_store_kv_name
}

output "cluster_secret_store_kv_ref" {
  description = "Reference object for the Vault KV ClusterSecretStore"
  value = {
    name = local.cluster_secret_store_kv_name
    kind = "ClusterSecretStore"
  }
}

output "approle_auth_secret_name" {
  description = "Name of the Kubernetes Secret storing AppRole credentials"
  value       = local.approle_auth_secret_name
}

output "approle_auth_secret_ref" {
  description = "Reference object for the AppRole credentials Secret"
  value = {
    name      = local.approle_auth_secret_name
    namespace = local.effective_namespace
  }
}

output "vault_address" {
  description = "Vault endpoint used by the secret stores"
  value       = local.vault_address
}

output "kv_mount_path" {
  description = "KV v2 mount path in Vault"
  value       = local.kv_mount_path
}

output "sync_policy_contract" {
  description = "Contract for downstream workloads to consume secrets from Vault"
  value = {
    kv_secret_store = {
      name       = local.cluster_secret_store_kv_name
      kind       = "ClusterSecretStore"
      mount_path = local.kv_mount_path
    }
    vault_address         = local.vault_address
    auth_secret_name      = local.approle_auth_secret_name
    auth_secret_namespace = local.effective_namespace
  }
}

output "rendered_manifests" {
  description = "Rendered Flux-ready manifests keyed by GitOps path (only populated when mode=render)"
  value       = local.is_render_mode ? local.rendered_manifests : {}
  sensitive   = true
}
