output "namespace" {
  description = "Namespace where Traefik is installed"
  value       = var.create_namespace ? kubernetes_namespace.traefik[0].metadata[0].name : var.namespace
}

output "helm_release_name" {
  description = "Name of the Traefik Helm release"
  value       = helm_release.traefik.name
}

output "cluster_issuer_name" {
  description = "Name of the ClusterIssuer for certificate management"
  value       = kubernetes_manifest.cluster_issuer.manifest.metadata.name
}

output "cluster_issuer_ref" {
  description = "Reference object for the ClusterIssuer suitable for use in Certificate resources"
  value = {
    name  = kubernetes_manifest.cluster_issuer.manifest.metadata.name
    kind  = "ClusterIssuer"
    group = "cert-manager.io"
  }
}

output "dashboard_hostname" {
  description = "Hostname for the Traefik dashboard (null if dashboard is disabled)"
  value       = var.dashboard_enabled ? var.dashboard_hostname : null
}

output "ingress_class_name" {
  description = "Name of the IngressClass created by Traefik"
  value       = var.ingress_class_name
}
