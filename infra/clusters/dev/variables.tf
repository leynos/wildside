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
}

variable "kubernetes_version" {
  type        = string
  description = "Exact Kubernetes version slug supported by DigitalOcean"
  default     = "1.33.1-do.3"
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
