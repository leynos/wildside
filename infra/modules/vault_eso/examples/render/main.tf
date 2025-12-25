# Render-only example for the vault_eso module.
#
# This example does not require cluster access. It exercises the module's
# render mode, which emits Flux-ready YAML manifests for GitOps workflows.

variable "mode" {
  description = "Module execution mode"
  type        = string
  default     = "render"
}

variable "vault_address" {
  description = "HTTPS endpoint of the external Vault appliance"
  type        = string
  default     = "https://vault.example.test:8200"
}

variable "vault_ca_bundle_pem" {
  description = "PEM-encoded CA certificate for Vault TLS"
  type        = string
  default     = <<-PEM
    -----BEGIN CERTIFICATE-----
    MIIBpTCCAQygAwIBAgIUT5h1rn5G7p1DqVtJtB/9z7xvvi4wCgYIKoZIzj0EAwIw
    EjEQMA4GA1UEAxMHVGVzdCBDQTAeFw0yNTAxMDEwMDAwMDBaFw0zNTAxMDEwMDAw
    MDBaMBIxEDAOBgNVBAMTB1Rlc3QgQ0EwWTATBgcqhkjOPQIBBggqhkjOPQMBBwNC
    AAR1F2xEtpKX8m+u1e2Oh0ObiPrbQGdQzYz9g9FZ2qfml7yPRxYzQ/1sV9UXiYdI
    9kRvuOOAvs0wrg9pF7lzo1MwUTAdBgNVHQ4EFgQUC+eP3RzL1Xlyx6r0fAF5Oe9k
    GtIwHwYDVR0jBBgwFoAUC+eP3RzL1Xlyx6r0fAF5Oe9kGtIwDwYDVR0TAQH/BAUw
    AwEB/zAKBggqhkjOPQQDAgNJADBGAiEA0fU1cXWm5E0zB2u7k+8F8K5L2D1W7KZf
    0Sg5n2O4V5oCIQC2Dg0mEC0J3r5a0T8G8WvM3mZ3P6o7xI8V/8b7nXw1Wg==
    -----END CERTIFICATE-----
    PEM
}

variable "approle_role_id" {
  description = "AppRole role_id for ESO authentication"
  type        = string
  default     = "test-role-id-12345678-1234-1234-1234-123456789012"
}

variable "approle_secret_id" {
  description = "AppRole secret_id for ESO authentication"
  type        = string
  default     = "test-secret-id-12345678-1234-1234-1234-123456789012"
  sensitive   = true
}

variable "namespace" {
  description = "Namespace where ESO will be installed"
  type        = string
  default     = "external-secrets"
}

variable "kv_mount_path" {
  description = "KV v2 mount path in Vault"
  type        = string
  default     = "secret"
}

variable "cluster_secret_store_kv_name" {
  description = "Name of the ClusterSecretStore for Vault KV v2"
  type        = string
  default     = "vault-kv"
}

variable "webhook_replica_count" {
  description = "Replica count for the ESO webhook"
  type        = number
  default     = 2
}

module "vault_eso" {
  source = "../.."

  mode = var.mode

  namespace           = var.namespace
  vault_address       = var.vault_address
  vault_ca_bundle_pem = var.vault_ca_bundle_pem
  approle_role_id     = var.approle_role_id
  approle_secret_id   = var.approle_secret_id
  kv_mount_path       = var.kv_mount_path

  cluster_secret_store_kv_name = var.cluster_secret_store_kv_name
  webhook_replica_count        = var.webhook_replica_count
}

output "rendered_manifests" {
  description = "Rendered manifests keyed by GitOps path"
  value       = module.vault_eso.rendered_manifests
  sensitive   = true
}

output "cluster_secret_store_kv_name" {
  description = "Name of the KV ClusterSecretStore"
  value       = module.vault_eso.cluster_secret_store_kv_name
}

output "sync_policy_contract" {
  description = "Contract for downstream workload consumption"
  value       = module.vault_eso.sync_policy_contract
}
