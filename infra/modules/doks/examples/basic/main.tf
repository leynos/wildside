module "doks" {
  source       = "../.."
  cluster_name = var.cluster_name
  region       = var.region
  kubernetes_version = var.kubernetes_version
  node_pools   = var.node_pools
}

variable "cluster_name" {
  type = string
}

variable "region" {
  type = string
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
}

variable "kubernetes_version" {
  type = string
}
