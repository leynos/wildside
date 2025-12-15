output "namespace" {
  description = "Namespace where Traefik is installed"
  value = (
    local.is_apply_mode && var.create_namespace ? kubernetes_namespace.traefik[0].metadata[0].name : local.namespace
  )
}

output "helm_release_name" {
  description = "Name of the Traefik Helm release"
  value       = local.helm_release_name
}

output "cluster_issuer_name" {
  description = "Name of the ClusterIssuer for certificate management"
  value       = local.cluster_issuer_name
}

output "cluster_issuer_ref" {
  description = "Reference object for the ClusterIssuer suitable for use in Certificate resources"
  value = {
    name  = local.cluster_issuer_name
    kind  = "ClusterIssuer"
    group = "cert-manager.io"
  }
}

output "dashboard_hostname" {
  description = "Hostname for the Traefik dashboard (null if dashboard is disabled)"
  value       = var.dashboard_enabled ? local.dashboard_hostname : null
}

output "dashboard_hostnames" {
  description = "Dashboard hostnames (empty unless the dashboard is enabled)"
  value = (
    var.dashboard_enabled && local.dashboard_hostname != null ? [local.dashboard_hostname] : []
  )
}

output "default_certificate_issuer_name" {
  description = "Name of the default certificate issuer for cluster workloads"
  value       = local.cluster_issuer_name
}

output "ingress_class_name" {
  description = "Name of the IngressClass created by Traefik"
  value       = local.ingress_class_name
}

output "rendered_manifests" {
  description = "Rendered Flux-ready manifests keyed by GitOps path (only populated when mode=render)"
  value       = local.is_render_mode ? local.rendered_manifests : {}
}
