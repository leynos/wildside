variable "webhook_release_enabled" {
  description = "Whether to deploy the Namecheap DNS-01 webhook Helm release"
  type        = bool
  default     = false
  nullable    = false
}

variable "webhook_release_name" {
  description = "Name assigned to the Namecheap webhook Helm release"
  type        = string
  default     = "cert-manager-webhook-namecheap"

  validation {
    condition     = var.webhook_release_name != null && length(trimspace(var.webhook_release_name)) > 0
    error_message = "webhook_release_name must not be blank"
  }
}

variable "webhook_helm_repository_name" {
  description = "Flux HelmRepository name for the Namecheap webhook chart"
  type        = string
  default     = "private-helm-repo"

  validation {
    condition     = var.webhook_helm_repository_name != null && length(trimspace(var.webhook_helm_repository_name)) > 0
    error_message = "webhook_helm_repository_name must not be blank"
  }
}

variable "webhook_repository_interval" {
  description = "Interval for the Namecheap webhook HelmRepository reconciliation"
  type        = string
  default     = "1h"

  validation {
    condition     = var.webhook_repository_interval != null && length(trimspace(var.webhook_repository_interval)) > 0
    error_message = "webhook_repository_interval must not be blank"
  }
}

variable "webhook_chart_repository" {
  description = "Helm repository hosting the Namecheap webhook chart"
  type        = string
  default     = null

  validation {
    condition = (
      !var.webhook_release_enabled ||
      (
        var.webhook_chart_repository != null &&
        length(trimspace(var.webhook_chart_repository)) > 0 &&
        (can(regex("^https://", trimspace(var.webhook_chart_repository))) ||
        can(regex("^oci://", trimspace(var.webhook_chart_repository))))
      )
    )
    error_message = "webhook_chart_repository must be an https:// or oci:// URL when webhook_release_enabled"
  }
}

variable "webhook_chart_name" {
  description = "Name of the Namecheap webhook chart"
  type        = string
  default     = "cert-manager-webhook-namecheap"

  validation {
    condition     = var.webhook_chart_name != null && length(trimspace(var.webhook_chart_name)) > 0
    error_message = "webhook_chart_name must not be blank"
  }
}

variable "webhook_chart_version" {
  description = "Chart version for the Namecheap webhook"
  type        = string
  default     = "0.2.0"

  validation {
    condition = (
      var.webhook_chart_version != null &&
      can(regex(
        "^[0-9]+\\.[0-9]+\\.[0-9]+(-[0-9A-Za-z.-]+)?(\\+[0-9A-Za-z.-]+)?$",
        trimspace(var.webhook_chart_version)
      ))
    )
    error_message = "webhook_chart_version must be a semantic version (e.g., 0.2.0)"
  }
}

variable "webhook_release_interval" {
  description = "Interval for the Namecheap webhook HelmRelease reconciliation"
  type        = string
  default     = "1h"

  validation {
    condition     = var.webhook_release_interval != null && length(trimspace(var.webhook_release_interval)) > 0
    error_message = "webhook_release_interval must not be blank"
  }
}

variable "webhook_repository_type" {
  description = "Optional repository type for the Namecheap webhook HelmRepository (oci)"
  type        = string
  default     = null

  validation {
    condition = (
      var.webhook_repository_type == null ||
      trimspace(var.webhook_repository_type) == "oci"
    )
    error_message = "webhook_repository_type must be 'oci' when set"
  }
}

variable "webhook_release_replica_count" {
  description = "Replica count for the Namecheap webhook"
  type        = number
  default     = 2
  nullable    = false

  validation {
    condition     = var.webhook_release_replica_count > 0
    error_message = "webhook_release_replica_count must be greater than zero"
  }
}
