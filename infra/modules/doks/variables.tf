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
    condition     = length(var.region) > 0
    error_message = "region must not be empty"
  }
}

variable "kubernetes_version" {
  description = "Kubernetes version for the cluster"
  type        = string
  validation {
    condition     = length(var.kubernetes_version) > 0
    error_message = "kubernetes_version must not be empty"
  }
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
    condition     = length(var.node_pools) > 0
    error_message = "at least one node pool is required"
  }
}
