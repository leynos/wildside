variable "kubeconfig_path" {
  description = "Path to a kubeconfig file with cluster-admin access"
  type        = string
  default     = null
  validation {
    condition     = length(trimspace(coalesce(var.kubeconfig_path, ""))) > 0
    error_message = "kubeconfig_path must point to a kubeconfig file for the target cluster"
  }
}

variable "namespace" {
  description = "Namespace where Flux will be installed"
  type        = string
  default     = "flux-system"
}

variable "git_repository_name" {
  description = "Name assigned to the Flux GitRepository resource"
  type        = string
  default     = "flux-system"
}

variable "kustomization_name" {
  description = "Name assigned to the Flux Kustomization resource"
  type        = string
  default     = "flux-system"
}

variable "git_repository_url" {
  description = "URL of the Git repository containing Flux manifests"
  type        = string
}

variable "git_repository_branch" {
  description = "Branch to synchronise"
  type        = string
  default     = "main"
}

variable "git_repository_path" {
  description = "Relative path within the Git repository"
  type        = string
}

variable "git_repository_secret_name" {
  description = "Optional Kubernetes secret with Git credentials"
  type        = string
  default     = null
}

variable "reconcile_interval" {
  description = "Sync interval for Flux reconciliation"
  type        = string
  default     = "1m"
}

variable "kustomization_prune" {
  description = "Whether Flux should prune removed resources"
  type        = bool
  default     = true
}

variable "kustomization_suspend" {
  description = "Whether to suspend the Flux Kustomization"
  type        = bool
  default     = false
}

variable "kustomization_timeout" {
  description = "Timeout for Flux Kustomization reconciliation"
  type        = string
  default     = "5m"
}

variable "helm_values" {
  description = "Inline YAML values passed to the Flux Helm release"
  type        = list(string)
  default     = []
}

variable "helm_values_files" {
  description = "Additional YAML files providing values for the Flux Helm release"
  type        = list(string)
  default     = []
}

locals {
  kubeconfig = trimspace(coalesce(var.kubeconfig_path, ""))
}

provider "kubernetes" {
  config_path = local.kubeconfig != "" ? local.kubeconfig : null
}

provider "helm" {
  kubernetes {
    config_path = local.kubeconfig != "" ? local.kubeconfig : null
  }
}

module "fluxcd" {
  source = "../.."

  namespace                  = var.namespace
  git_repository_name        = var.git_repository_name
  kustomization_name         = var.kustomization_name
  git_repository_url         = var.git_repository_url
  git_repository_branch      = var.git_repository_branch
  git_repository_path        = var.git_repository_path
  git_repository_secret_name = var.git_repository_secret_name
  reconcile_interval         = var.reconcile_interval
  kustomization_prune        = var.kustomization_prune
  kustomization_suspend      = var.kustomization_suspend
  kustomization_timeout      = var.kustomization_timeout
  helm_values                = var.helm_values
  helm_values_files          = var.helm_values_files
}

output "namespace" {
  description = "Namespace where Flux is installed"
  value       = module.fluxcd.namespace
}

output "git_repository_name" {
  description = "Name of the managed Flux GitRepository"
  value       = module.fluxcd.git_repository_name
}

output "kustomization_name" {
  description = "Name of the managed Flux Kustomization"
  value       = module.fluxcd.kustomization_name
}
