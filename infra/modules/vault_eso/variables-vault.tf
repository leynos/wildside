variable "vault_address" {
  description = "HTTPS endpoint of the external Vault appliance"
  type        = string

  validation {
    condition = (
      var.vault_address != null &&
      length(trimspace(var.vault_address)) > 0 &&
      can(regex("^https://", trimspace(var.vault_address)))
    )
    error_message = "vault_address must be an https:// URL"
  }
}

variable "vault_ca_bundle_pem" {
  description = "PEM-encoded CA certificate for validating the Vault TLS endpoint"
  type        = string

  validation {
    condition = (
      var.vault_ca_bundle_pem != null &&
      length(trimspace(var.vault_ca_bundle_pem)) > 0 &&
      can(regex("-----BEGIN CERTIFICATE-----", var.vault_ca_bundle_pem))
    )
    error_message = "vault_ca_bundle_pem must be a valid PEM-encoded certificate"
  }
}

variable "approle_role_id" {
  description = "AppRole role_id for ESO authentication to Vault"
  type        = string

  validation {
    condition     = var.approle_role_id != null && length(trimspace(var.approle_role_id)) > 0
    error_message = "approle_role_id must not be blank"
  }
}

variable "approle_secret_id" {
  description = "AppRole secret_id for ESO authentication to Vault"
  type        = string
  sensitive   = true

  validation {
    condition     = var.approle_secret_id != null && length(trimspace(var.approle_secret_id)) > 0
    error_message = "approle_secret_id must not be blank"
  }
}

variable "approle_auth_secret_name" {
  description = "Name of the Kubernetes Secret storing AppRole credentials"
  type        = string
  default     = "vault-approle-credentials"

  validation {
    condition = (
      var.approle_auth_secret_name != null &&
      length(trimspace(var.approle_auth_secret_name)) > 0 &&
      length(trimspace(var.approle_auth_secret_name)) <= 253 &&
      can(regex("^[a-z0-9]([-a-z0-9]*[a-z0-9])?$", trimspace(var.approle_auth_secret_name)))
    )
    error_message = "approle_auth_secret_name must be a valid Kubernetes Secret name"
  }
}

variable "approle_mount_path" {
  description = "Mount path for the AppRole auth method in Vault"
  type        = string
  default     = "approle"

  validation {
    condition     = var.approle_mount_path != null && length(trimspace(var.approle_mount_path)) > 0
    error_message = "approle_mount_path must not be blank"
  }
}

variable "kv_mount_path" {
  description = "Mount path for the KV v2 secrets engine in Vault"
  type        = string
  default     = "secret"

  validation {
    condition     = var.kv_mount_path != null && length(trimspace(var.kv_mount_path)) > 0
    error_message = "kv_mount_path must not be blank"
  }
}

variable "pki_enabled" {
  description = "Whether to create a ClusterSecretStore for the Vault PKI engine"
  type        = bool
  default     = false
  nullable    = false
}

variable "pki_mount_path" {
  description = "Mount path for the PKI secrets engine in Vault"
  type        = string
  default     = "pki"

  validation {
    condition     = var.pki_mount_path != null && length(trimspace(var.pki_mount_path)) > 0
    error_message = "pki_mount_path must not be blank"
  }
}
