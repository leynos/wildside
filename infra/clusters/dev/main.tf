# Root configuration for the dev DigitalOcean Kubernetes cluster.
# Uses the reusable `doks` module and optionally installs Flux for GitOps.

provider "digitalocean" {}

locals {
  tags_normalised = distinct([for t in var.tags : trimspace(t)])
  node_pools_normalised = [
    for np in var.node_pools : merge(
      np,
      {
        tags = try(distinct([for t in np.tags : trimspace(t)]), null)
      }
    )
  ]
}

module "doks" {
  count             = var.should_create_cluster ? 1 : 0
  source            = "../../modules/doks"
  cluster_name      = var.cluster_name
  region            = var.region
  node_pools        = local.node_pools_normalised
  tags              = local.tags_normalised
  expose_kubeconfig = var.expose_kubeconfig
}

locals {
  flux_kubeconfig_path = trimspace(var.flux_kubeconfig_path)
  doks_cluster_ids     = try([for m in module.doks : m.cluster_id], [])
  doks_cluster_id      = length(local.doks_cluster_ids) > 0 ? local.doks_cluster_ids[0] : null
  should_fetch_cluster = var.should_install_flux && local.flux_kubeconfig_path == "" && local.doks_cluster_id != null
}

data "digitalocean_kubernetes_cluster" "flux" {
  count = local.should_fetch_cluster ? 1 : 0
  name  = var.cluster_name
}

locals {
  flux_cluster = local.flux_kubeconfig_path == "" ? try(data.digitalocean_kubernetes_cluster.flux[0], null) : null
  flux_host    = local.flux_kubeconfig_path == "" ? try(local.flux_cluster.endpoint, null) : null
  flux_token   = local.flux_kubeconfig_path == "" ? try(local.flux_cluster.kube_config[0].token, null) : null
  flux_ca_cert = local.flux_kubeconfig_path == "" ? try(base64decode(local.flux_cluster.kube_config[0].cluster_ca_certificate), null) : null
}

provider "kubernetes" {
  alias                  = "flux"
  host                   = local.flux_kubeconfig_path == "" ? local.flux_host : null
  token                  = local.flux_kubeconfig_path == "" ? local.flux_token : null
  cluster_ca_certificate = local.flux_kubeconfig_path == "" ? local.flux_ca_cert : null
  config_path            = local.flux_kubeconfig_path != "" ? local.flux_kubeconfig_path : null
}

provider "helm" {
  alias = "flux"

  kubernetes {
    host                   = local.flux_kubeconfig_path == "" ? local.flux_host : null
    token                  = local.flux_kubeconfig_path == "" ? local.flux_token : null
    cluster_ca_certificate = local.flux_kubeconfig_path == "" ? local.flux_ca_cert : null
    config_path            = local.flux_kubeconfig_path != "" ? local.flux_kubeconfig_path : null
  }
}

module "fluxcd" {
  count  = var.should_install_flux && (local.flux_kubeconfig_path != "" || var.should_create_cluster) ? 1 : 0
  source = "../../modules/fluxcd"

  providers = {
    kubernetes = kubernetes.flux
    helm       = helm.flux
  }

  lifecycle {
    precondition {
      condition     = local.flux_kubeconfig_path != "" || var.should_create_cluster
      error_message = "Flux install requires either flux_kubeconfig_path or should_create_cluster=true."
    }
  }

  namespace                  = var.flux_namespace
  git_repository_name        = var.flux_git_repository_name
  kustomization_name         = var.flux_kustomization_name
  git_repository_url         = var.flux_git_repository_url
  git_repository_branch      = var.flux_git_repository_branch
  git_repository_path        = var.flux_git_repository_path
  git_repository_secret_name = var.flux_git_repository_secret_name
  reconcile_interval         = var.flux_reconcile_interval
  kustomization_prune        = var.flux_kustomization_prune
  kustomization_suspend      = var.flux_kustomization_suspend
  kustomization_timeout      = var.flux_kustomization_timeout
}

check "flux_authentication_source" {
  assert {
    condition     = !var.should_install_flux || local.flux_kubeconfig_path != "" || var.should_create_cluster
    error_message = "Flux install requires either flux_kubeconfig_path to be set or should_create_cluster=true."
  }
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

output "flux_namespace" {
  description = "Namespace where Flux is installed"
  value       = var.should_install_flux ? module.fluxcd[0].namespace : null
}

output "flux_git_repository_name" {
  description = "Name of the Flux GitRepository resource"
  value       = var.should_install_flux ? module.fluxcd[0].git_repository_name : null
}

output "flux_kustomization_name" {
  description = "Name of the Flux Kustomization resource"
  value       = var.should_install_flux ? module.fluxcd[0].kustomization_name : null
}
