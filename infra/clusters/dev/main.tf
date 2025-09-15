# Root configuration for the dev DigitalOcean Kubernetes cluster.
# Uses the reusable `doks` module.

provider "digitalocean" {}

locals {
  tags_normalised = [for t in var.tags : trimspace(t)]
}

module "doks" {
  count              = var.should_create_cluster ? 1 : 0
  source             = "../../modules/doks"
  cluster_name       = var.cluster_name
  region             = var.region
  kubernetes_version = var.kubernetes_version
  node_pools         = var.node_pools
  tags               = local.tags_normalised
  expose_kubeconfig  = var.expose_kubeconfig
}

output "cluster_id" {
  description = "Cluster ID from module"
  value       = var.should_create_cluster ? module.doks[0].cluster_id : null
}

output "endpoint" {
  description = "Cluster API endpoint from module"
  value       = var.should_create_cluster ? module.doks[0].endpoint : null
}

output "kubeconfig" {
  description = "Kubeconfig from module"
  value       = var.should_create_cluster && var.expose_kubeconfig ? module.doks[0].kubeconfig : null
  sensitive   = true
}
