# Apply-mode example for the vault_eso module.
#
# This example deploys External Secrets Operator directly to a Kubernetes
# cluster and configures ClusterSecretStore resources. It requires a kubeconfig
# with cluster-admin access.

variable "kubeconfig_path" {
  description = "Path to a kubeconfig file with cluster-admin access"
  type        = string
  default     = null
  validation {
    condition = (
      var.kubeconfig_path == null ||
      (
        trimspace(var.kubeconfig_path) != "" &&
        fileexists(trimspace(var.kubeconfig_path))
      )
    )
    error_message = "Set kubeconfig_path to a readable kubeconfig file before running the example"
  }
}

variable "vault_address" {
  description = "HTTPS endpoint of the external Vault appliance"
  type        = string

  validation {
    condition     = var.vault_address != null && can(regex("^https://", trimspace(var.vault_address)))
    error_message = "vault_address must be an https:// URL"
  }
}

variable "vault_ca_bundle_pem" {
  description = "PEM-encoded CA certificate for Vault TLS"
  type        = string

  validation {
    condition     = var.vault_ca_bundle_pem != null && length(trimspace(var.vault_ca_bundle_pem)) > 0
    error_message = "vault_ca_bundle_pem must be a non-empty string"
  }
}

variable "approle_role_id" {
  description = "AppRole role_id for ESO authentication"
  type        = string

  validation {
    condition     = var.approle_role_id != null && length(trimspace(var.approle_role_id)) > 0
    error_message = "approle_role_id must be a non-empty string"
  }
}

variable "approle_secret_id" {
  description = "AppRole secret_id for ESO authentication"
  type        = string
  sensitive   = true

  validation {
    condition     = var.approle_secret_id != null && length(trimspace(var.approle_secret_id)) > 0
    error_message = "approle_secret_id must be a non-empty string"
  }
}

# Optional variables with defaults matching the module. These defaults
# intentionally duplicate the module's defaults to enable test flexibility.

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

variable "pki_enabled" {
  description = "Whether to create a ClusterSecretStore for Vault PKI"
  type        = bool
  default     = false
}

variable "pki_mount_path" {
  description = "PKI mount path in Vault"
  type        = string
  default     = "pki"
}

variable "cluster_secret_store_kv_name" {
  description = "Name of the ClusterSecretStore for Vault KV v2"
  type        = string
  default     = "vault-kv"
}

variable "cluster_secret_store_pki_name" {
  description = "Name of the ClusterSecretStore for Vault PKI"
  type        = string
  default     = "vault-pki"
}

provider "kubernetes" {
  config_path = var.kubeconfig_path != null ? trimspace(var.kubeconfig_path) : null
}

provider "helm" {
  kubernetes {
    config_path = var.kubeconfig_path != null ? trimspace(var.kubeconfig_path) : null
  }
}

module "vault_eso" {
  source = "../.."

  mode = "apply"

  namespace           = var.namespace
  vault_address       = var.vault_address
  vault_ca_bundle_pem = var.vault_ca_bundle_pem
  approle_role_id     = var.approle_role_id
  approle_secret_id   = var.approle_secret_id
  kv_mount_path       = var.kv_mount_path

  pki_enabled                   = var.pki_enabled
  pki_mount_path                = var.pki_mount_path
  cluster_secret_store_kv_name  = var.cluster_secret_store_kv_name
  cluster_secret_store_pki_name = var.cluster_secret_store_pki_name
}

output "namespace" {
  description = "Namespace where ESO is installed"
  value       = module.vault_eso.namespace
}

output "cluster_secret_store_kv_name" {
  description = "Name of the KV ClusterSecretStore"
  value       = module.vault_eso.cluster_secret_store_kv_name
}

output "cluster_secret_store_pki_name" {
  description = "Name of the PKI ClusterSecretStore"
  value       = module.vault_eso.cluster_secret_store_pki_name
}

output "sync_policy_contract" {
  description = "Contract for downstream workload consumption"
  value       = module.vault_eso.sync_policy_contract
}
