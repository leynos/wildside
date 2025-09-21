variable "namespace" {
  description = "Namespace where Flux controllers and GitOps objects are created"
  type        = string
  default     = "flux-system"

  validation {
    condition = (
      length(trimspace(var.namespace)) > 0 &&
      length(trimspace(var.namespace)) <= 63 &&
      can(regex("^[a-z0-9]([-a-z0-9]*[a-z0-9])?$", var.namespace))
    )
    error_message = "namespace must be a valid Kubernetes namespace name"
  }
}

variable "git_repository_name" {
  description = "Name assigned to the Flux GitRepository resource"
  type        = string
  default     = "flux-system"

  validation {
    condition     = length(trimspace(var.git_repository_name)) > 0
    error_message = "git_repository_name must not be blank"
  }
}

variable "kustomization_name" {
  description = "Name assigned to the Flux Kustomization resource"
  type        = string
  default     = "flux-system"

  validation {
    condition     = length(trimspace(var.kustomization_name)) > 0
    error_message = "kustomization_name must not be blank"
  }
}

variable "git_repository_url" {
  description = "URL of the Git repository containing Flux manifests. Must be an HTTPS or SSH endpoint."
  type        = string

  validation {
    condition = (
      length(trimspace(var.git_repository_url)) > 0 &&
      can(regex("^(https://|ssh://|git@)", var.git_repository_url))
    )
    error_message = "git_repository_url must be a non-empty HTTPS, SSH, or git@ URL"
  }
}

variable "git_repository_branch" {
  description = "Branch of the Git repository to sync"
  type        = string
  default     = "main"

  validation {
    condition     = length(trimspace(var.git_repository_branch)) > 0
    error_message = "git_repository_branch must not be blank"
  }
}

variable "git_repository_path" {
  description = "Path within the Git repository that contains the root Flux kustomization"
  type        = string

  validation {
    condition = (
      length(trimspace(var.git_repository_path)) > 0 &&
      (
        trimspace(var.git_repository_path) == "." ||
        startswith(trimspace(var.git_repository_path), "./")
      )
      && length(regexall("\\.\\.", trimspace(var.git_repository_path))) == 0
    )
    error_message = "git_repository_path must be a non-empty relative path without traversal"
  }
}

variable "git_repository_secret_name" {
  description = "Optional Kubernetes secret name providing Git credentials"
  type        = string
  default     = null

  validation {
    condition = (
      var.git_repository_secret_name == null ||
      length(trimspace(var.git_repository_secret_name)) > 0
    )
    error_message = "git_repository_secret_name must not be blank when set"
  }
}

variable "reconcile_interval" {
  description = "Sync interval used by Flux GitRepository and Kustomization resources"
  type        = string
  default     = "1m"

  validation {
    condition     = can(regex("^([0-9]+(s|m|h|d|w))+$", trimspace(var.reconcile_interval)))
    error_message = "reconcile_interval must be a Go duration string such as 30s, 5m, or 1h30m"
  }
}

variable "kustomization_prune" {
  description = "Whether Flux should prune resources removed from the Git repository"
  type        = bool
  default     = true
}

variable "kustomization_suspend" {
  description = "Whether to suspend reconciliation of the Kustomization"
  type        = bool
  default     = false
}

variable "kustomization_timeout" {
  description = "Maximum time Flux waits for Kustomization reconciliation"
  type        = string
  default     = "5m"

  validation {
    condition     = can(regex("^([0-9]+(s|m|h|d|w))+$", trimspace(var.kustomization_timeout)))
    error_message = "kustomization_timeout must be a valid Go duration string"
  }
}

variable "chart_repository" {
  description = "Helm repository hosting the flux2 chart"
  type        = string
  default     = "https://fluxcd-community.github.io/helm-charts"

  validation {
    condition = (
      length(trimspace(var.chart_repository)) > 0 &&
      can(regex("^https://", var.chart_repository))
    )
    error_message = "chart_repository must be an HTTPS URL"
  }
}

variable "chart_name" {
  description = "Name of the Helm chart used to install Flux"
  type        = string
  default     = "flux2"

  validation {
    condition     = length(trimspace(var.chart_name)) > 0
    error_message = "chart_name must not be blank"
  }
}

variable "chart_version" {
  description = "Exact Helm chart version used to install Flux"
  type        = string
  default     = "2.16.4"

  validation {
    condition     = length(trimspace(var.chart_version)) > 0
    error_message = "chart_version must not be blank"
  }
}

variable "helm_release_name" {
  description = "Name assigned to the Flux Helm release"
  type        = string
  default     = "flux-system"

  validation {
    condition     = length(trimspace(var.helm_release_name)) > 0
    error_message = "helm_release_name must not be blank"
  }
}

variable "helm_wait" {
  description = "Whether to wait for the Helm release to report success"
  type        = bool
  default     = true
}

variable "helm_timeout" {
  description = "Timeout (in seconds) for the Helm release operation"
  type        = number
  default     = 600

  validation {
    condition     = var.helm_timeout > 0
    error_message = "helm_timeout must be a positive number of seconds"
  }
}

variable "helm_values" {
  description = "Inline YAML values passed to the Flux Helm release"
  type        = list(string)
  default     = []
}

variable "helm_values_files" {
  description = "Additional YAML files providing values for the Flux Helm release"
  type        = list(string)
  default     = []

  validation {
    condition = alltrue([
      for path in var.helm_values_files : length(trimspace(path)) > 0
    ])
    error_message = "helm_values_files must not contain blank file paths"
  }
}
