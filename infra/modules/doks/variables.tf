variable "cluster_name" {
  description = "Name of the Kubernetes cluster"
  type        = string
  validation {
    condition     = length(var.cluster_name) > 0
    error_message = "cluster_name must not be empty"
  }
}

variable "region" {
  description = "DigitalOcean region for the cluster"
  type        = string
  validation {
    condition     = can(regex("^[a-z]{3}\\d$", var.region))
    error_message = "region must be a valid DigitalOcean slug (e.g., nyc1)"
  }
}

variable "kubernetes_version" {
  description = "Kubernetes version for the cluster"
  type        = string
  validation {
    condition     = can(regex("^\\d+\\.\\d+\\.\\d+(-do\\.\\d+)?$", var.kubernetes_version))
    error_message = "kubernetes_version must match X.Y.Z or X.Y.Z-do.N (DigitalOcean format)"
  }
}

variable "tags" {
  description = "Tags applied to the Kubernetes cluster"
  type        = list(string)
  default     = []
}

variable "node_pools" {
  description = "Configuration for cluster node pools"
  type = list(object({
    name       = string
    size       = string
    node_count = number
    auto_scale = bool
    min_nodes  = number
    max_nodes  = number
    tags       = optional(list(string))
  }))
  validation {
    condition = length(var.node_pools) > 0 && alltrue([
      for np in var.node_pools :
      np.node_count >= 1 &&
      np.min_nodes >= 1 &&
      np.max_nodes >= np.min_nodes
    ])
    error_message = "each node pool requires at least one node with min_nodes >= 1 and max_nodes >= min_nodes"
  }
}

variable "expose_kubeconfig" {
  description = "Expose kubeconfig via outputs (stores credentials in state). Use only for local/dev."
  type        = bool
  default     = false
}
