variable "cluster_secret_store_kv_name" {
  description = "Name of the ClusterSecretStore for Vault KV v2 engine"
  type        = string
  default     = "vault-kv"

  validation {
    condition = (
      var.cluster_secret_store_kv_name != null &&
      length(trimspace(var.cluster_secret_store_kv_name)) > 0 &&
      length(trimspace(var.cluster_secret_store_kv_name)) <= 253 &&
      can(regex("^[a-z0-9]([-a-z0-9]*[a-z0-9])?$", trimspace(var.cluster_secret_store_kv_name)))
    )
    error_message = "cluster_secret_store_kv_name must be a valid Kubernetes resource name"
  }
}

variable "cluster_secret_store_pki_name" {
  description = "Name of the ClusterSecretStore for Vault PKI engine"
  type        = string
  default     = "vault-pki"

  validation {
    condition = (
      var.cluster_secret_store_pki_name != null &&
      length(trimspace(var.cluster_secret_store_pki_name)) > 0 &&
      length(trimspace(var.cluster_secret_store_pki_name)) <= 253 &&
      can(regex("^[a-z0-9]([-a-z0-9]*[a-z0-9])?$", trimspace(var.cluster_secret_store_pki_name)))
    )
    error_message = "cluster_secret_store_pki_name must be a valid Kubernetes resource name"
  }
}

variable "secret_store_retry_max_attempts" {
  description = "Maximum number of retry attempts for secret store operations"
  type        = number
  default     = 5
  nullable    = false

  validation {
    condition     = var.secret_store_retry_max_attempts >= 0
    error_message = "secret_store_retry_max_attempts must be non-negative"
  }
}

variable "secret_store_retry_interval" {
  description = "Interval between retry attempts for secret store operations"
  type        = string
  default     = "10s"

  validation {
    condition = (
      var.secret_store_retry_interval != null &&
      length(trimspace(var.secret_store_retry_interval)) > 0
    )
    error_message = "secret_store_retry_interval must not be blank"
  }
}
