# Input variables for the dev cluster root configuration.
# Defaults provision a small two-node cluster in nyc1 for preview workloads.

variable "should_create_cluster" {
  type        = bool
  description = "Whether to create the dev cluster"
  default     = false
}

variable "cluster_name" {
  type        = string
  description = "Name for the DOKS cluster"
  default     = "wildside-dev"

  validation {
    condition     = length(trimspace(var.cluster_name)) > 0
    error_message = "cluster_name must not be blank"
  }
}

variable "region" {
  type        = string
  description = "DigitalOcean region slug"
  default     = "nyc1"

  validation {
    condition     = can(regex("^[a-z]{3}[0-9]$", var.region))
    error_message = "region must be a 3-letter region code followed by a digit (e.g., nyc1, sfo3, ams3)."
  }
}

variable "node_pools" {
  type = list(object({
    name       = string
    size       = string
    node_count = number
    auto_scale = bool
    min_nodes  = number
    max_nodes  = number
    tags       = optional(list(string))
  }))
  description = "List of node pool definitions"

  validation {
    condition = alltrue([
      for p in var.node_pools :
      p.node_count >= 2 &&
      (
        p.auto_scale
        ? (
          p.min_nodes >= 2 &&
          p.max_nodes >= p.min_nodes &&
          p.node_count >= p.min_nodes &&
          p.node_count <= p.max_nodes
        )
        : (
          p.min_nodes == p.node_count &&
          p.max_nodes == p.node_count
        )
      )
    ])
    error_message = "Each node pool must have at least 2 nodes. When auto_scale is true, min_nodes must be less than or equal to node_count, and node_count must be between min_nodes and max_nodes. When auto_scale is false, min_nodes and max_nodes must equal node_count."
  }
  default = [
    {
      name       = "default"
      size       = "s-2vcpu-2gb"
      node_count = 2
      auto_scale = false
      min_nodes  = 2
      max_nodes  = 2
      tags       = ["env:dev"]
    }
  ]
}

variable "tags" {
  type        = list(string)
  description = "Tags applied to the cluster"
  default     = ["env:dev"]

  validation {
    condition     = length(var.tags) > 0
    error_message = "tags must contain at least one value"
  }

  validation {
    condition     = alltrue([for t in var.tags : length(trimspace(t)) > 0])
    error_message = "tags must not be blank strings"
  }

  validation {
    condition     = length(distinct([for t in var.tags : trimspace(t)])) == length(var.tags)
    error_message = "tags must be unique after trimming whitespace"
  }
}

variable "expose_kubeconfig" {
  type        = bool
  description = "Expose kubeconfig via module outputs (stores credentials in state)"
  default     = false
}

variable "should_install_flux" {
  type        = bool
  description = "Whether to install FluxCD on the cluster"
  default     = false

  validation {
    condition = (
      !var.should_install_flux ||
      var.should_create_cluster ||
      length(trimspace(var.flux_kubeconfig_path)) > 0
    )
    error_message = "should_install_flux requires should_create_cluster to be true or flux_kubeconfig_path to be set"
  }
}

variable "flux_namespace" {
  type        = string
  description = "Namespace where Flux resources will be installed"
  default     = "flux-system"

  validation {
    condition = (
      length(trimspace(var.flux_namespace)) > 0 &&
      can(regex("^[a-z0-9]([-a-z0-9]*[a-z0-9])?$", var.flux_namespace))
    )
    error_message = "flux_namespace must be a valid Kubernetes namespace name"
  }
}

variable "flux_git_repository_name" {
  type        = string
  description = "Name for the Flux GitRepository resource"
  default     = "flux-system"

  validation {
    condition     = length(trimspace(var.flux_git_repository_name)) > 0
    error_message = "flux_git_repository_name must not be blank"
  }
}

variable "flux_kustomization_name" {
  type        = string
  description = "Name for the Flux Kustomization resource"
  default     = "flux-system"

  validation {
    condition     = length(trimspace(var.flux_kustomization_name)) > 0
    error_message = "flux_kustomization_name must not be blank"
  }
}

variable "flux_git_repository_url" {
  type        = string
  description = "URL of the Git repository Flux should watch"
  default     = null

  validation {
    condition = (
      !var.should_install_flux || (
        length(trimspace(coalesce(var.flux_git_repository_url, ""))) > 0 &&
        can(regex("^(https://|ssh://|git@|file://)", coalesce(var.flux_git_repository_url, "")))
      )
    )
    error_message = "flux_git_repository_url must be set to an HTTPS, SSH, git@, or file URL when installing Flux"
  }
}

variable "flux_git_repository_branch" {
  type        = string
  description = "Git branch Flux should reconcile"
  default     = "main"

  validation {
    condition     = (!var.should_install_flux) || length(trimspace(var.flux_git_repository_branch)) > 0
    error_message = "flux_git_repository_branch must not be blank when installing Flux"
  }
}

variable "flux_git_repository_path" {
  type        = string
  description = "Relative path within the Git repository that hosts the cluster manifests"
  default     = "clusters/dev"

  validation {
    condition = (
      !var.should_install_flux || (
        length(trimspace(var.flux_git_repository_path)) > 0 &&
        !startswith(trimspace(var.flux_git_repository_path), "/")
      )
    )
    error_message = "flux_git_repository_path must be a non-empty relative path when installing Flux"
  }
}

variable "flux_git_repository_secret_name" {
  type        = string
  description = "Optional Kubernetes secret containing Git credentials"
  default     = null

  validation {
    condition = (
      var.flux_git_repository_secret_name == null || length(trimspace(var.flux_git_repository_secret_name)) > 0
    )
    error_message = "flux_git_repository_secret_name must not be blank when set"
  }
}

variable "flux_reconcile_interval" {
  type        = string
  description = "Flux reconciliation interval"
  default     = "1m"

  validation {
    condition     = (!var.should_install_flux) || can(regex("^([0-9]+(s|m|h|d|w))+$", trimspace(var.flux_reconcile_interval)))
    error_message = "flux_reconcile_interval must be a valid duration string"
  }
}

variable "flux_kustomization_prune" {
  type        = bool
  description = "Whether Flux should prune deleted resources"
  default     = true
}

variable "flux_kustomization_suspend" {
  type        = bool
  description = "Whether to suspend Flux reconciliation"
  default     = false
}

variable "flux_kustomization_timeout" {
  type        = string
  description = "Timeout for Flux Kustomization reconciliation"
  default     = "5m"

  validation {
    condition     = (!var.should_install_flux) || can(regex("^([0-9]+(s|m|h|d|w))+$", trimspace(var.flux_kustomization_timeout)))
    error_message = "flux_kustomization_timeout must be a valid duration string"
  }
}

variable "flux_kubeconfig_path" {
  type        = string
  description = "Optional path to an existing kubeconfig for Flux installation"
  default     = ""
}
