variable "kubeconfig_path" {
  description = "Path to a kubeconfig file with cluster-admin access"
  type        = string
  default     = null
  validation {
    condition = (
      var.kubeconfig_path != null &&
      trimspace(var.kubeconfig_path) != "" &&
      fileexists(trimspace(var.kubeconfig_path))
    )
    error_message = "Set kubeconfig_path to a readable kubeconfig file before running the example"
  }
}

variable "namespace" {
  description = "Namespace where Traefik will be installed"
  type        = string
  default     = "traefik"
}

variable "acme_email" {
  description = "Email address registered with ACME certificate authority"
  type        = string
}

variable "cloudflare_api_token_secret_name" {
  description = "Name of the Kubernetes secret containing the Cloudflare API token"
  type        = string
}

variable "cloudflare_api_token_secret_key" {
  description = "Key within the Cloudflare API token secret"
  type        = string
  default     = "token"
}

variable "cluster_issuer_name" {
  description = "Name of the ClusterIssuer resource"
  type        = string
  default     = "letsencrypt-prod"
}

variable "acme_server" {
  description = "ACME server URL (production or staging)"
  type        = string
  default     = "https://acme-v02.api.letsencrypt.org/directory"
}

variable "chart_version" {
  description = "Traefik Helm chart version"
  type        = string
  default     = "25.0.3"
}

variable "dashboard_enabled" {
  description = "Whether to enable the Traefik dashboard"
  type        = bool
  default     = false
}

variable "dashboard_hostname" {
  description = "Hostname for the Traefik dashboard"
  type        = string
  default     = null
}

variable "helm_values" {
  description = "Inline YAML values passed to the Traefik Helm release"
  type        = list(string)
  default     = []
}

variable "helm_values_files" {
  description = "Additional YAML files providing values for the Traefik Helm release"
  type        = list(string)
  default     = []
}

locals {
  # coalesce rejects empty strings; use a single space so trimspace still normalises to blank.
  kubeconfig = trimspace(coalesce(var.kubeconfig_path, " "))
}

provider "kubernetes" {
  config_path = local.kubeconfig != "" ? local.kubeconfig : null
}

provider "helm" {
  kubernetes {
    config_path = local.kubeconfig != "" ? local.kubeconfig : null
  }
}

module "traefik" {
  source = "../.."

  namespace                        = var.namespace
  acme_email                       = var.acme_email
  cloudflare_api_token_secret_name = var.cloudflare_api_token_secret_name
  cloudflare_api_token_secret_key  = var.cloudflare_api_token_secret_key
  cluster_issuer_name              = var.cluster_issuer_name
  acme_server                      = var.acme_server
  chart_version                    = var.chart_version
  dashboard_enabled                = var.dashboard_enabled
  dashboard_hostname               = var.dashboard_hostname
  helm_values                      = var.helm_values
  helm_values_files                = var.helm_values_files
}

output "namespace" {
  description = "Namespace where Traefik is installed"
  value       = module.traefik.namespace
}

output "helm_release_name" {
  description = "Name of the Traefik Helm release"
  value       = module.traefik.helm_release_name
}

output "cluster_issuer_name" {
  description = "Name of the ClusterIssuer"
  value       = module.traefik.cluster_issuer_name
}

output "cluster_issuer_ref" {
  description = "Reference object for the ClusterIssuer"
  value       = module.traefik.cluster_issuer_ref
}

output "ingress_class_name" {
  description = "Name of the IngressClass created by Traefik"
  value       = module.traefik.ingress_class_name
}

output "dashboard_hostname" {
  description = "Hostname for the Traefik dashboard"
  value       = module.traefik.dashboard_hostname
}
