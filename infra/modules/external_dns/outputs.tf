output "namespace" {
  description = "Namespace where ExternalDNS is installed"
  value       = local.effective_namespace
}

output "helm_release_name" {
  description = "Name of the ExternalDNS Helm release"
  value       = local.helm_release_name
}

output "txt_owner_id" {
  description = "Unique identifier for ExternalDNS ownership TXT records"
  value       = local.txt_owner_id
}

output "domain_filters" {
  description = "List of DNS domains managed by ExternalDNS"
  value       = local.domain_filters
}

output "policy" {
  description = "DNS record management policy (sync or upsert-only)"
  value       = local.policy
}

output "sources" {
  description = "Kubernetes resource types watched by ExternalDNS"
  value       = local.sources
}

output "cloudflare_proxied" {
  description = "Whether Cloudflare proxy is enabled by default for DNS records"
  value       = var.cloudflare_proxied
}

output "rendered_manifests" {
  description = "Rendered Flux-ready manifests keyed by GitOps path (only populated when mode=render)"
  value       = local.is_render_mode ? local.rendered_manifests : {}
}
