# Render-only example for the ExternalDNS module.
#
# This example does not require cluster access. It exercises the module's
# "render" mode, which emits Flux-ready YAML manifests for the GitOps
# repository.
#
# Most variables use module defaults. Override them as needed.

variable "domain_filters" {
  description = "List of DNS domains that ExternalDNS should manage"
  type        = list(string)
  default     = ["example.test"]
}

variable "txt_owner_id" {
  description = "Unique identifier for ExternalDNS ownership TXT records"
  type        = string
  default     = "render-example"
}

variable "cloudflare_api_token_secret_name" {
  description = "Name of the Kubernetes secret containing the Cloudflare API token"
  type        = string
  default     = "cloudflare-api-token"
}

# Optional overrides - tests set these via -var flags
variable "cloudflare_proxied" {
  description = "Whether to enable Cloudflare proxy by default"
  type        = bool
  default     = false
}

variable "zone_id_filter" {
  description = "Optional map of domain names to Cloudflare zone IDs"
  type        = map(string)
  default     = {}

  validation {
    condition = alltrue([
      for domain, zone_id in var.zone_id_filter : (
        can(regex("^([a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?\\.)+[a-z]{2,}$", lower(trimspace(domain)))) &&
        can(regex("^[a-f0-9]{32}$", lower(trimspace(zone_id))))
      )
    ])
    error_message = "zone_id_filter keys must be valid domains; values must be 32-char hex Cloudflare zone IDs"
  }
}

module "external_dns" {
  source = "../.."

  mode = "render"

  domain_filters                   = var.domain_filters
  txt_owner_id                     = var.txt_owner_id
  cloudflare_api_token_secret_name = var.cloudflare_api_token_secret_name
  cloudflare_proxied               = var.cloudflare_proxied
  zone_id_filter                   = var.zone_id_filter
}

output "rendered_manifests" {
  description = "Rendered manifests keyed by GitOps path"
  value       = module.external_dns.rendered_manifests
}

output "namespace" {
  description = "Namespace where ExternalDNS will be installed"
  value       = module.external_dns.namespace
}

output "txt_owner_id" {
  description = "Unique identifier for ExternalDNS ownership TXT records"
  value       = module.external_dns.txt_owner_id
}

output "domain_filters" {
  description = "List of DNS domains managed by ExternalDNS"
  value       = module.external_dns.domain_filters
}

output "zone_id_filter" {
  description = "Map of domain names to Cloudflare zone IDs"
  value       = module.external_dns.zone_id_filter
}

output "managed_zones" {
  description = "Unified zone configuration: domain -> zone_id"
  value       = module.external_dns.managed_zones
}
