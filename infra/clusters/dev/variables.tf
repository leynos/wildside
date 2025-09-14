# Input variables for the dev cluster root configuration.
# Defaults provision a small two-node cluster in nyc1 for preview workloads.

variable "cluster_name" {
  type        = string
  description = "Name for the DOKS cluster"
  default     = "wildside-dev"
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

variable "kubernetes_version" {
  type        = string
  description = "Exact Kubernetes version slug supported by DigitalOcean"
  default     = ""

  validation {
    condition     = var.kubernetes_version == "" || can(regex("^[0-9]+\\.[0-9]+\\.[0-9]+-do\\.[0-9]+$", var.kubernetes_version))
    error_message = "kubernetes_version may be empty or must match '<major>.<minor>.<patch>-do.<n>' (e.g., '1.33.1-do.3')."
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
          p.node_count <= p.max_nodes &&
          p.min_nodes <= p.node_count
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
}

variable "expose_kubeconfig" {
  type        = bool
  description = "Expose kubeconfig via module outputs (stores credentials in state)"
  default     = false
}
