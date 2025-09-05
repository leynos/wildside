module "doks" {
  source             = "../.."
  cluster_name       = var.cluster_name
  region             = var.region
  kubernetes_version = var.kubernetes_version
  node_pools         = var.node_pools
  tags               = var.tags
}

variable "cluster_name" {
  type        = string
  description = "Name for the DOKS cluster"
}

variable "region" {
  type        = string
  description = "DigitalOcean region (e.g., nyc1, sfo3)"
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
}

variable "kubernetes_version" {
  type        = string
  description = "Exact Kubernetes version slug supported by DigitalOcean (e.g., 1.28.0-do.0)"
}

variable "tags" {
  type        = list(string)
  description = "Tags applied to the cluster"
  default     = []
}

output "example_cluster_id" {
  description = "Cluster ID from module"
  value       = module.doks.cluster_id
}

output "example_cluster_endpoint" {
  description = "Cluster API endpoint from module"
  value       = module.doks.endpoint
}
