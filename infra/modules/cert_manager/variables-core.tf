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

variable "namespace" {
  description = "Namespace where cert-manager will be installed"
  type        = string
  default     = "cert-manager"

  validation {
    condition = (
      var.namespace != null &&
      length(trimspace(var.namespace)) > 0 &&
      length(trimspace(var.namespace)) <= 63 &&
      can(regex("^[a-z0-9]([-a-z0-9]*[a-z0-9])?$", trimspace(var.namespace)))
    )
    error_message = "namespace must be a valid Kubernetes namespace name"
  }
}

variable "create_namespace" {
  description = "Whether the module should create the cert-manager namespace"
  type        = bool
  default     = true
  nullable    = false
}

variable "helm_release_name" {
  description = "Name assigned to the cert-manager Helm release"
  type        = string
  default     = "cert-manager"

  validation {
    condition     = var.helm_release_name != null && length(trimspace(var.helm_release_name)) > 0
    error_message = "helm_release_name must not be blank"
  }
}

variable "chart_repository" {
  description = "Helm repository hosting the cert-manager chart"
  type        = string
  default     = "oci://quay.io/jetstack/charts"

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
  description = "Name of the Helm chart used to install cert-manager"
  type        = string
  default     = "cert-manager"

  validation {
    condition     = var.chart_name != null && length(trimspace(var.chart_name)) > 0
    error_message = "chart_name must not be blank"
  }
}

variable "chart_version" {
  description = "Exact Helm chart version for cert-manager"
  type        = string
  default     = "v1.18.2"

  validation {
    condition = (
      var.chart_version != null &&
      can(regex(
        "^v?[0-9]+\\.[0-9]+\\.[0-9]+(-[0-9A-Za-z.-]+)?(\\+[0-9A-Za-z.-]+)?$",
        trimspace(var.chart_version)
      ))
    )
    error_message = "chart_version must be a semantic version (e.g., v1.18.2)"
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
  description = "Inline YAML values passed to the cert-manager Helm release"
  type        = list(string)
  default     = []
  nullable    = false
}

variable "helm_values_files" {
  description = "Additional YAML files providing values for cert-manager"
  type        = list(string)
  default     = []

  validation {
    condition = (
      var.helm_values_files != null &&
      alltrue([
        for path in var.helm_values_files : path != null && length(trimspace(path)) > 0
      ])
    )
    error_message = "helm_values_files must not contain blank file paths"
  }
}

variable "install_crds" {
  description = "Whether to install cert-manager CRDs via Helm"
  type        = bool
  default     = true
  nullable    = false
}

variable "controller_replica_count" {
  description = "Replica count for the cert-manager controller"
  type        = number
  default     = 3
  nullable    = false

  validation {
    condition     = var.controller_replica_count > 0
    error_message = "controller_replica_count must be greater than zero"
  }
}

variable "controller_resources" {
  description = "Resource requests and limits for the cert-manager controller"
  type = object({
    requests = map(string)
    limits   = map(string)
  })
  default = {
    requests = {
      cpu    = "100m"
      memory = "128Mi"
    }
    limits = {
      cpu    = "250m"
      memory = "256Mi"
    }
  }
  nullable = false
}

variable "webhook_replica_count" {
  description = "Replica count for the cert-manager webhook"
  type        = number
  default     = 3
  nullable    = false

  validation {
    condition     = var.webhook_replica_count > 0
    error_message = "webhook_replica_count must be greater than zero"
  }
}

variable "webhook_resources" {
  description = "Resource requests and limits for the cert-manager webhook"
  type = object({
    requests = map(string)
    limits   = map(string)
  })
  default = {
    requests = {
      cpu    = "50m"
      memory = "64Mi"
    }
    limits = {
      cpu    = "200m"
      memory = "128Mi"
    }
  }
  nullable = false
}

variable "cainjector_replica_count" {
  description = "Replica count for the cert-manager cainjector"
  type        = number
  default     = 3
  nullable    = false

  validation {
    condition     = var.cainjector_replica_count > 0
    error_message = "cainjector_replica_count must be greater than zero"
  }
}

variable "cainjector_resources" {
  description = "Resource requests and limits for the cert-manager cainjector"
  type = object({
    requests = map(string)
    limits   = map(string)
  })
  default = {
    requests = {
      cpu    = "50m"
      memory = "64Mi"
    }
    limits = {
      cpu    = "200m"
      memory = "128Mi"
    }
  }
  nullable = false
}

variable "pdb_enabled" {
  description = "Whether to render/apply PodDisruptionBudgets for cert-manager"
  type        = bool
  default     = true
  nullable    = false
}

variable "pdb_min_available" {
  description = "Minimum available pods for cert-manager webhook/cainjector PDBs"
  type        = number
  default     = 2
  nullable    = false

  validation {
    condition     = var.pdb_min_available > 0
    error_message = "pdb_min_available must be greater than zero"
  }
}

variable "webhook_pdb_name" {
  description = "Name of the PodDisruptionBudget for cert-manager webhook"
  type        = string
  default     = "cert-manager-webhook-pdb"

  validation {
    condition     = var.webhook_pdb_name != null && length(trimspace(var.webhook_pdb_name)) > 0
    error_message = "webhook_pdb_name must not be blank"
  }
}

variable "cainjector_pdb_name" {
  description = "Name of the PodDisruptionBudget for cert-manager cainjector"
  type        = string
  default     = "cert-manager-cainjector-pdb"

  validation {
    condition     = var.cainjector_pdb_name != null && length(trimspace(var.cainjector_pdb_name)) > 0
    error_message = "cainjector_pdb_name must not be blank"
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
  description = "Flux HelmRepository name for the cert-manager chart"
  type        = string
  default     = "jetstack"

  validation {
    condition     = var.flux_helm_repository_name != null && length(trimspace(var.flux_helm_repository_name)) > 0
    error_message = "flux_helm_repository_name must not be blank"
  }
}

variable "flux_helm_repository_interval" {
  description = "Interval for the Flux HelmRepository reconciliation"
  type        = string
  default     = "24h"

  validation {
    condition     = var.flux_helm_repository_interval != null && length(trimspace(var.flux_helm_repository_interval)) > 0
    error_message = "flux_helm_repository_interval must not be blank"
  }
}

variable "flux_helm_release_interval" {
  description = "Interval for the Flux HelmRelease reconciliation"
  type        = string
  default     = "1h"

  validation {
    condition     = var.flux_helm_release_interval != null && length(trimspace(var.flux_helm_release_interval)) > 0
    error_message = "flux_helm_release_interval must not be blank"
  }
}
