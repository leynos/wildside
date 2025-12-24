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
  description = "Interval between retry attempts for secret store operations (Go duration format)"
  type        = string
  default     = "10s"

  validation {
    condition = (
      var.secret_store_retry_interval != null &&
      can(regex("^([0-9]+(ns|us|µs|ms|s|m|h))+$", trimspace(var.secret_store_retry_interval)))
    )
    error_message = "secret_store_retry_interval must be a valid Go duration (e.g., 10s, 1m30s, 500ms)"
  }
}
