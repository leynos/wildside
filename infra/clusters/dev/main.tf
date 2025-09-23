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
  flux_config = {
    install           = var.flux.install
    kubeconfig_path   = trimspace(var.flux.kubeconfig_path)
    allow_file_scheme = var.flux.allow_file_scheme
    namespace         = trimspace(var.flux.namespace)
    git_repository = {
      name        = trimspace(var.flux.git_repository.name)
      url         = var.flux.git_repository.url == null ? null : trimspace(var.flux.git_repository.url)
      branch      = trimspace(var.flux.git_repository.branch)
      path        = trimspace(var.flux.git_repository.path)
      secret_name = var.flux.git_repository.secret_name == null ? null : trimspace(var.flux.git_repository.secret_name)
    }
    reconcile_interval = trimspace(var.flux.reconcile_interval)
    kustomization = {
      name    = trimspace(var.flux.kustomization.name)
      prune   = var.flux.kustomization.prune
      suspend = var.flux.kustomization.suspend
      timeout = trimspace(var.flux.kustomization.timeout)
    }
    helm = {
      release_name = trimspace(var.flux.helm.release_name)
      repository   = trimspace(var.flux.helm.repository)
      chart        = trimspace(var.flux.helm.chart)
      version      = trimspace(var.flux.helm.version)
      wait         = var.flux.helm.wait
      timeout      = var.flux.helm.timeout
      values       = var.flux.helm.values
      values_files = [for path in var.flux.helm.values_files : trimspace(path)]
    }
  }
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
  flux_install_requested          = local.flux_config.install
  flux_kubeconfig_path            = local.flux_config.kubeconfig_path
  flux_using_kubeconfig           = local.flux_kubeconfig_path != ""
  flux_module_creates_cluster     = var.should_create_cluster
  flux_auth_source_available      = local.flux_using_kubeconfig || local.flux_module_creates_cluster
  should_fetch_cluster            = local.flux_install_requested && !local.flux_using_kubeconfig && local.flux_module_creates_cluster
  should_configure_flux_providers = local.flux_install_requested && local.flux_auth_source_available
}

data "digitalocean_kubernetes_cluster" "flux" {
  count      = local.should_fetch_cluster ? 1 : 0
  cluster_id = module.doks[count.index].cluster_id
}

locals {
  flux_cluster = local.should_fetch_cluster ? try(data.digitalocean_kubernetes_cluster.flux[0], null) : null
  flux_host    = local.should_fetch_cluster ? try(local.flux_cluster.endpoint, null) : null
  flux_token   = local.should_fetch_cluster ? try(local.flux_cluster.kube_config[0].token, null) : null
  flux_ca_cert = local.should_fetch_cluster ? try(base64decode(local.flux_cluster.kube_config[0].cluster_ca_certificate), null) : null
  flux_provider_auth = !local.should_configure_flux_providers ? null : local.flux_using_kubeconfig ? {
    host                   = null
    token                  = null
    cluster_ca_certificate = null
    config_path            = local.flux_kubeconfig_path
    } : {
    host                   = local.flux_host
    token                  = local.flux_token
    cluster_ca_certificate = local.flux_ca_cert
    config_path            = null
  }
}

provider "kubernetes" {
  alias                  = "flux"
  host                   = try(local.flux_provider_auth.host, null)
  token                  = try(local.flux_provider_auth.token, null)
  cluster_ca_certificate = try(local.flux_provider_auth.cluster_ca_certificate, null)
  config_path            = try(local.flux_provider_auth.config_path, null)
}

provider "helm" {
  alias = "flux"

  kubernetes {
    host                   = try(local.flux_provider_auth.host, null)
    token                  = try(local.flux_provider_auth.token, null)
    cluster_ca_certificate = try(local.flux_provider_auth.cluster_ca_certificate, null)
    config_path            = try(local.flux_provider_auth.config_path, null)
  }
}

module "fluxcd" {
  count  = local.should_configure_flux_providers ? 1 : 0
  source = "../../modules/fluxcd"

  providers = {
    kubernetes = kubernetes.flux
    helm       = helm.flux
  }

  namespace                  = local.flux_config.namespace
  git_repository_name        = local.flux_config.git_repository.name
  kustomization_name         = local.flux_config.kustomization.name
  git_repository_url         = local.flux_config.git_repository.url
  git_repository_branch      = local.flux_config.git_repository.branch
  git_repository_path        = local.flux_config.git_repository.path
  git_repository_secret_name = local.flux_config.git_repository.secret_name
  reconcile_interval         = local.flux_config.reconcile_interval
  kustomization_prune        = local.flux_config.kustomization.prune
  kustomization_suspend      = local.flux_config.kustomization.suspend
  kustomization_timeout      = local.flux_config.kustomization.timeout
  helm_release_name          = local.flux_config.helm.release_name
  chart_repository           = local.flux_config.helm.repository
  chart_name                 = local.flux_config.helm.chart
  chart_version              = local.flux_config.helm.version
  helm_wait                  = local.flux_config.helm.wait
  helm_timeout               = local.flux_config.helm.timeout
  helm_values                = local.flux_config.helm.values
  helm_values_files          = local.flux_config.helm.values_files
  allow_file_scheme          = local.flux_config.allow_file_scheme
}

check "flux_authentication_source" {
  assert {
    condition     = !local.flux_config.install || local.flux_auth_source_available
    error_message = "Flux install requires flux.kubeconfig_path or should_create_cluster=true to provide credentials. Create the cluster first, then re-apply with kubeconfig configured."
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
  value       = length(module.fluxcd) > 0 ? module.fluxcd[0].namespace : null
}

output "flux_git_repository_name" {
  description = "Name of the Flux GitRepository resource"
  value       = length(module.fluxcd) > 0 ? module.fluxcd[0].git_repository_name : null
}

output "flux_kustomization_name" {
  description = "Name of the Flux Kustomization resource"
  value       = length(module.fluxcd) > 0 ? module.fluxcd[0].kustomization_name : null
}
