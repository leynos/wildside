//! Apply-mode example for the ExternalDNS module.
//!
//! This example deploys ExternalDNS directly to a Kubernetes cluster. It
//! requires a kubeconfig with cluster-admin access.
//!
//! Most variables use module defaults. Override them as needed for specific
//! deployments.

variable "kubeconfig_path" {
  description = "Path to a kubeconfig file with cluster-admin access"
  type        = string
  default     = null
  validation {
    condition = (
      var.kubeconfig_path == null ||
      (
        trimspace(var.kubeconfig_path) != "" &&
        fileexists(trimspace(var.kubeconfig_path))
      )
    )
    error_message = "Set kubeconfig_path to a readable kubeconfig file before running the example"
  }
}

variable "domain_filters" {
  description = "List of DNS domains that ExternalDNS should manage"
  type        = list(string)
}

variable "txt_owner_id" {
  description = "Unique identifier for ExternalDNS ownership TXT records"
  type        = string
}

variable "cloudflare_api_token_secret_name" {
  description = "Name of the Kubernetes secret containing the Cloudflare API token"
  type        = string
}

# Optional test overrides - use specific defaults, tests can override via -var
variable "namespace" {
  description = "Namespace where ExternalDNS will be installed"
  type        = string
  default     = "external-dns"
}

variable "cloudflare_api_token_secret_key" {
  description = "Key within the Cloudflare API token secret"
  type        = string
  default     = "token"
}

variable "chart_repository" {
  description = "Helm repository hosting the ExternalDNS chart"
  type        = string
  default     = "https://kubernetes-sigs.github.io/external-dns/"
}

variable "chart_name" {
  description = "Name of the Helm chart"
  type        = string
  default     = "external-dns"
}

variable "chart_version" {
  description = "ExternalDNS Helm chart version"
  type        = string
  default     = "1.16.1"
}

variable "helm_release_name" {
  description = "Name assigned to the Helm release"
  type        = string
  default     = "external-dns"
}

variable "helm_values_files" {
  description = "Additional YAML files providing values"
  type        = list(string)
  default     = []
}

variable "helm_values" {
  description = "Inline YAML values"
  type        = list(string)
  default     = []
}

variable "policy" {
  description = "DNS record management policy"
  type        = string
  default     = "sync"
}

variable "log_level" {
  description = "Log verbosity level"
  type        = string
  default     = "info"
}

variable "interval" {
  description = "Sync interval"
  type        = string
  default     = "1m"
}

provider "kubernetes" {
  config_path = var.kubeconfig_path != null ? trimspace(var.kubeconfig_path) : null
}

provider "helm" {
  kubernetes {
    config_path = var.kubeconfig_path != null ? trimspace(var.kubeconfig_path) : null
  }
}

module "external_dns" {
  source = "../.."

  mode = "apply"

  domain_filters                   = var.domain_filters
  txt_owner_id                     = var.txt_owner_id
  cloudflare_api_token_secret_name = var.cloudflare_api_token_secret_name
  namespace                        = var.namespace
  cloudflare_api_token_secret_key  = var.cloudflare_api_token_secret_key
  chart_repository                 = var.chart_repository
  chart_name                       = var.chart_name
  chart_version                    = var.chart_version
  helm_release_name                = var.helm_release_name
  helm_values_files                = var.helm_values_files
  helm_values                      = var.helm_values
  policy                           = var.policy
  log_level                        = var.log_level
  interval                         = var.interval
}

output "namespace" {
  description = "Namespace where ExternalDNS is installed"
  value       = module.external_dns.namespace
}

output "helm_release_name" {
  description = "Name of the ExternalDNS Helm release"
  value       = module.external_dns.helm_release_name
}

output "txt_owner_id" {
  description = "Unique identifier for ExternalDNS ownership TXT records"
  value       = module.external_dns.txt_owner_id
}

output "domain_filters" {
  description = "List of DNS domains managed by ExternalDNS"
  value       = module.external_dns.domain_filters
}

output "policy" {
  description = "DNS record management policy"
  value       = module.external_dns.policy
}

output "sources" {
  description = "Kubernetes resource types watched by ExternalDNS"
  value       = module.external_dns.sources
}

output "cloudflare_proxied" {
  description = "Whether Cloudflare proxy is enabled by default"
  value       = module.external_dns.cloudflare_proxied
}
