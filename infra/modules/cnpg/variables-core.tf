variable "mode" {
  description = "Whether the module should render Flux manifests (render) or apply resources directly (apply)"
  type        = string
  default     = "render"
  nullable    = false

  validation {
    condition     = contains(["render", "apply"], trimspace(var.mode))
    error_message = "mode must be one of: render, apply"
  }
}

variable "operator_namespace" {
  description = "Namespace where CloudNativePG operator will be installed"
  type        = string
  default     = "cnpg-system"

  validation {
    condition = (
      var.operator_namespace != null &&
      length(trimspace(var.operator_namespace)) > 0 &&
      length(trimspace(var.operator_namespace)) <= 63 &&
      can(regex("^[a-z0-9]([-a-z0-9]*[a-z0-9])?$", trimspace(var.operator_namespace)))
    )
    error_message = "operator_namespace must be a valid Kubernetes namespace name"
  }
}

variable "cluster_namespace" {
  description = "Namespace where the PostgreSQL cluster will run"
  type        = string
  default     = "databases"

  validation {
    condition = (
      var.cluster_namespace != null &&
      length(trimspace(var.cluster_namespace)) > 0 &&
      length(trimspace(var.cluster_namespace)) <= 63 &&
      can(regex("^[a-z0-9]([-a-z0-9]*[a-z0-9])?$", trimspace(var.cluster_namespace)))
    )
    error_message = "cluster_namespace must be a valid Kubernetes namespace name"
  }
}

variable "create_namespaces" {
  description = "Whether the module should create the operator and cluster namespaces"
  type        = bool
  default     = true
  nullable    = false
}

variable "helm_release_name" {
  description = "Name assigned to the CloudNativePG operator Helm release"
  type        = string
  default     = "cloudnative-pg"

  validation {
    condition     = var.helm_release_name != null && length(trimspace(var.helm_release_name)) > 0
    error_message = "helm_release_name must not be blank"
  }
}

variable "chart_repository" {
  description = "Helm repository hosting the cloudnative-pg chart"
  type        = string
  default     = "https://cloudnative-pg.github.io/charts"

  validation {
    condition = (
      var.chart_repository != null &&
      length(trimspace(var.chart_repository)) > 0 &&
      (can(regex("^https://", trimspace(var.chart_repository))) ||
      can(regex("^oci://", trimspace(var.chart_repository))))
    )
    error_message = "chart_repository must be an https:// or oci:// URL"
  }
}

variable "chart_name" {
  description = "Name of the Helm chart used to install CloudNativePG operator"
  type        = string
  default     = "cloudnative-pg"

  validation {
    condition     = var.chart_name != null && length(trimspace(var.chart_name)) > 0
    error_message = "chart_name must not be blank"
  }
}

variable "chart_version" {
  description = "Exact Helm chart version for CloudNativePG operator"
  type        = string
  default     = "0.23.2"

  validation {
    condition = (
      var.chart_version != null &&
      can(regex(
        "^v?[0-9]+\\.[0-9]+\\.[0-9]+(-[0-9A-Za-z.-]+)?(\\+[0-9A-Za-z.-]+)?$",
        trimspace(var.chart_version)
      ))
    )
    error_message = "chart_version must be a semantic version (e.g., 0.23.2)"
  }
}

variable "helm_wait" {
  description = "Whether to wait for the Helm release to report success"
  type        = bool
  default     = true
  nullable    = false
}

variable "helm_timeout" {
  description = "Timeout (in seconds) for the Helm release operation"
  type        = number
  default     = 600
  nullable    = false

  validation {
    condition     = var.helm_timeout > 0
    error_message = "helm_timeout must be a positive number of seconds"
  }
}

variable "helm_values" {
  description = "Inline YAML values passed to the CloudNativePG operator Helm release"
  type        = list(string)
  default     = []
  nullable    = false

  validation {
    condition = alltrue([
      for v in var.helm_values : can(yamldecode(v))
    ])
    error_message = "All helm_values entries must be valid YAML"
  }
}

variable "flux_namespace" {
  description = "Namespace where Flux controllers and sources run (render mode)"
  type        = string
  default     = "flux-system"

  validation {
    condition     = var.flux_namespace != null && length(trimspace(var.flux_namespace)) > 0
    error_message = "flux_namespace must not be blank"
  }
}

variable "flux_helm_repository_name" {
  description = "Flux HelmRepository name for the cloudnative-pg chart"
  type        = string
  default     = "cloudnative-pg"

  validation {
    condition     = var.flux_helm_repository_name != null && length(trimspace(var.flux_helm_repository_name)) > 0
    error_message = "flux_helm_repository_name must not be blank"
  }
}

variable "flux_helm_repository_interval" {
  description = "Interval for the Flux HelmRepository reconciliation (Go duration format)"
  type        = string
  default     = "24h"

  validation {
    condition = (
      var.flux_helm_repository_interval != null &&
      can(regex("^([0-9]+(ns|us|µs|ms|s|m|h))+$", trimspace(var.flux_helm_repository_interval)))
    )
    error_message = "flux_helm_repository_interval must be a valid Go duration (e.g., 1h, 24h, 30m)"
  }
}

variable "flux_helm_release_interval" {
  description = "Interval for the Flux HelmRelease reconciliation (Go duration format)"
  type        = string
  default     = "1h"

  validation {
    condition = (
      var.flux_helm_release_interval != null &&
      can(regex("^([0-9]+(ns|us|µs|ms|s|m|h))+$", trimspace(var.flux_helm_release_interval)))
    )
    error_message = "flux_helm_release_interval must be a valid Go duration (e.g., 1h, 24h, 30m)"
  }
}
