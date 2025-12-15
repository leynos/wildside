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

variable "namespace" {
  description = "Namespace where Traefik will be installed"
  type        = string
  default     = "traefik"
}

variable "chart_repository" {
  description = "Helm repository hosting the Traefik chart"
  type        = string
  default     = "https://traefik.github.io/charts"
}

variable "chart_name" {
  description = "Name of the Helm chart used to install Traefik"
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
  default     = "37.4.0"
}

variable "helm_release_name" {
  description = "Name assigned to the Traefik Helm release"
  type        = string
  default     = "traefik"
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

variable "service_type" {
  description = "Kubernetes service type for Traefik (LoadBalancer, ClusterIP, or NodePort)"
  type        = string
  default     = "LoadBalancer"
}

variable "external_traffic_policy" {
  description = "External traffic policy for LoadBalancer service (Local preserves client IPs)"
  type        = string
  default     = "Local"
}

variable "ingress_class_name" {
  description = "Name of the IngressClass created by Traefik"
  type        = string
  default     = "traefik"
}

variable "http_to_https_redirect" {
  description = "Whether to redirect HTTP traffic to HTTPS"
  type        = bool
  default     = true
}

variable "prometheus_metrics_enabled" {
  description = "Whether to enable Prometheus metrics endpoint"
  type        = bool
  default     = true
}

variable "service_monitor_enabled" {
  description = "Whether to create a ServiceMonitor for Prometheus Operator"
  type        = bool
  default     = true
}

provider "kubernetes" {
  config_path = var.kubeconfig_path != null ? trimspace(var.kubeconfig_path) : null
}

provider "helm" {
  kubernetes {
    config_path = var.kubeconfig_path != null ? trimspace(var.kubeconfig_path) : null
  }
}

module "traefik" {
  source = "../.."

  mode = "apply"

  namespace                        = var.namespace
  chart_repository                 = var.chart_repository
  chart_name                       = var.chart_name
  acme_email                       = var.acme_email
  cloudflare_api_token_secret_name = var.cloudflare_api_token_secret_name
  cloudflare_api_token_secret_key  = var.cloudflare_api_token_secret_key
  cluster_issuer_name              = var.cluster_issuer_name
  acme_server                      = var.acme_server
  chart_version                    = var.chart_version
  helm_release_name                = var.helm_release_name
  dashboard_enabled                = var.dashboard_enabled
  dashboard_hostname               = var.dashboard_hostname
  helm_values                      = var.helm_values
  helm_values_files                = var.helm_values_files
  service_type                     = var.service_type
  external_traffic_policy          = var.external_traffic_policy
  ingress_class_name               = var.ingress_class_name
  http_to_https_redirect           = var.http_to_https_redirect
  prometheus_metrics_enabled       = var.prometheus_metrics_enabled
  service_monitor_enabled          = var.service_monitor_enabled
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
