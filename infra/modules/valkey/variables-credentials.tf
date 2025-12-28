variable "anonymous_auth" {
  description = "Allow anonymous (unauthenticated) access to Valkey"
  type        = bool
  default     = false
  nullable    = false
}

variable "password_secret_name" {
  description = "Name of the Kubernetes Secret containing the Valkey password"
  type        = string
  default     = "valkey-password"

  validation {
    condition = (
      var.password_secret_name != null &&
      length(trimspace(var.password_secret_name)) > 0 &&
      length(trimspace(var.password_secret_name)) <= 253 &&
      can(regex("^[a-z0-9]([-a-z0-9]*[a-z0-9])?(\\.[a-z0-9]([-a-z0-9]*[a-z0-9])?)*$", trimspace(var.password_secret_name)))
    )
    error_message = "password_secret_name must be a valid Kubernetes Secret name"
  }
}

variable "password_secret_key" {
  description = "Key within the password Secret that holds the Valkey password"
  type        = string
  default     = "password"

  validation {
    condition     = var.password_secret_key != null && length(trimspace(var.password_secret_key)) > 0
    error_message = "password_secret_key must not be blank"
  }
}

variable "password_inline" {
  description = "Inline password value (only used when eso_enabled is false and a Secret is not pre-existing)"
  type        = string
  default     = ""
  sensitive   = true
}

variable "eso_enabled" {
  description = "Enable External Secrets Operator integration for password management"
  type        = bool
  default     = false
  nullable    = false
}

variable "eso_cluster_secret_store_name" {
  description = "Name of the ClusterSecretStore for ESO integration (from vault_eso module)"
  type        = string
  default     = "vault-backend"

  validation {
    condition     = var.eso_cluster_secret_store_name != null && length(trimspace(var.eso_cluster_secret_store_name)) > 0
    error_message = "eso_cluster_secret_store_name must not be blank"
  }
}

variable "eso_refresh_interval" {
  description = "Refresh interval for ExternalSecret synchronisation (Go duration format)"
  type        = string
  default     = "1h"

  validation {
    condition = (
      var.eso_refresh_interval != null &&
      can(regex("^([0-9]+(ns|us|µs|ms|s|m|h))+$", trimspace(var.eso_refresh_interval)))
    )
    error_message = "eso_refresh_interval must be a valid Go duration (e.g., 1h, 30m)"
  }
}

variable "password_vault_path" {
  description = "Vault KV path for the Valkey password (used when eso_enabled is true)"
  type        = string
  default     = ""
}

variable "password_vault_key" {
  description = "Key within the Vault secret that holds the password"
  type        = string
  default     = "password"

  validation {
    condition     = var.password_vault_key != null && length(trimspace(var.password_vault_key)) > 0
    error_message = "password_vault_key must not be blank"
  }
}
