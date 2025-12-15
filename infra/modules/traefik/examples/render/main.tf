//! Render-only example for the Traefik gateway module.
//!
//! This example does not require cluster access. It exercises the module's
//! "render" mode, which emits Flux-ready YAML manifests for the GitOps
//! repository.

variable "namespace" {
  description = "Namespace where Traefik will be deployed by Flux"
  type        = string
  default     = "traefik"
}

variable "acme_email" {
  description = "Email address registered with ACME certificate authority"
  type        = string
  default     = "admin@example.test"
}

variable "cloudflare_api_token_secret_name" {
  description = "Name of the Kubernetes secret containing the Cloudflare API token"
  type        = string
  default     = "cloudflare-api-token"
}

variable "service_annotations" {
  description = "Service annotations for Traefik's LoadBalancer service"
  type        = map(string)
  default     = {}
}

module "traefik" {
  source = "../.."

  mode = "render"

  namespace                        = var.namespace
  acme_email                       = var.acme_email
  cloudflare_api_token_secret_name = var.cloudflare_api_token_secret_name
  service_annotations              = var.service_annotations
}

output "rendered_manifests" {
  description = "Rendered manifests keyed by GitOps path"
  value       = module.traefik.rendered_manifests
}

output "default_certificate_issuer_name" {
  description = "Default certificate issuer name to use for workloads"
  value       = module.traefik.default_certificate_issuer_name
}

output "dashboard_hostnames" {
  description = "Dashboard hostnames (empty unless enabled)"
  value       = module.traefik.dashboard_hostnames
}
