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
    # coalesce ignores empty strings; use whitespace so nulls normalise to blank after trim.
    kubeconfig_path   = trimspace(coalesce(var.flux.kubeconfig_path, " "))
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
  # Respect the caller's `flux.install` flag without re-deriving the value in
  # downstream expressions. Keeping the flag named here makes the intent
  # obvious when referenced elsewhere.
  flux_install_requested = local.flux_config.install

  # Store the trimmed kubeconfig once so the subsequent checks read clearly and
  # avoid repeating the normalisation logic inline with boolean operations.
  flux_kubeconfig_path = local.flux_config.kubeconfig_path

  # Flux can only talk to the cluster when a kubeconfig path survives
  # normalisation. An empty string means we have no credentials to work with.
  flux_using_kubeconfig = local.flux_kubeconfig_path != ""

  # We only render Flux resources when the caller requested an install and we
  # have an authentication source (currently kubeconfig). This keeps the
  # provider configuration evaluable at plan time and prevents misconfigured
  # resources from being created.
  should_configure_flux = local.flux_install_requested && local.flux_using_kubeconfig
}

provider "kubernetes" {
  alias       = "flux"
  config_path = local.flux_using_kubeconfig ? local.flux_kubeconfig_path : null
}

provider "helm" {
  alias = "flux"

  kubernetes {
    config_path = local.flux_using_kubeconfig ? local.flux_kubeconfig_path : null
  }
}

module "fluxcd" {
  count  = local.should_configure_flux ? 1 : 0
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
    condition     = !local.flux_config.install || local.flux_using_kubeconfig
    error_message = "Flux install requires flux.kubeconfig_path to reference a readable kubeconfig. Create the cluster first, export its credentials, then re-apply with kubeconfig configured."
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
