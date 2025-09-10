# Root configuration for the dev DigitalOcean Kubernetes cluster.
# Uses the reusable `doks` module.

provider "digitalocean" {}

module "doks" {
  source             = "../../modules/doks"
  cluster_name       = var.cluster_name
  region             = var.region
  kubernetes_version = var.kubernetes_version
  node_pools         = var.node_pools
  tags               = var.tags
  expose_kubeconfig  = var.expose_kubeconfig
}

output "cluster_id" {
  description = "Cluster ID from module"
  value       = module.doks.cluster_id
}

output "endpoint" {
  description = "Cluster API endpoint from module"
  value       = module.doks.endpoint
}

output "kubeconfig" {
  description = "Kubeconfig from module"
  value       = module.doks.kubeconfig
  sensitive   = true
}
