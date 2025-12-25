variable "eso_enabled" {
  description = "Whether to create ExternalSecret resources for credential management via ESO"
  type        = bool
  default     = false
  nullable    = false
}

variable "eso_cluster_secret_store_name" {
  description = "Name of the ClusterSecretStore created by the vault_eso module"
  type        = string
  default     = "vault-kv"

  validation {
    condition = (
      var.eso_cluster_secret_store_name != null &&
      length(trimspace(var.eso_cluster_secret_store_name)) > 0
    )
    error_message = "eso_cluster_secret_store_name must not be blank"
  }
}

variable "eso_refresh_interval" {
  description = "Refresh interval for ExternalSecret resources (Go duration format)"
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

variable "superuser_credentials_vault_path" {
  description = "Vault KV path for PostgreSQL superuser credentials"
  type        = string
  default     = ""
}

variable "superuser_credentials_secret_name" {
  description = "Kubernetes Secret name for PostgreSQL superuser credentials"
  type        = string
  default     = "cnpg-superuser-credentials"

  validation {
    condition = (
      var.superuser_credentials_secret_name != null &&
      length(trimspace(var.superuser_credentials_secret_name)) > 0
    )
    error_message = "superuser_credentials_secret_name must not be blank"
  }
}

variable "app_credentials_vault_path" {
  description = "Vault KV path for application database credentials"
  type        = string
  default     = ""
}

variable "app_credentials_secret_name" {
  description = "Kubernetes Secret name for application database credentials"
  type        = string
  default     = "cnpg-app-credentials"

  validation {
    condition = (
      var.app_credentials_secret_name != null &&
      length(trimspace(var.app_credentials_secret_name)) > 0
    )
    error_message = "app_credentials_secret_name must not be blank"
  }
}

variable "backup_credentials_vault_path" {
  description = "Vault KV path for S3 backup credentials (alternative to inline credentials)"
  type        = string
  default     = ""
}
