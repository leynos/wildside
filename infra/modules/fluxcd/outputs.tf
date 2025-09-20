output "namespace" {
  description = "Namespace where Flux components are installed"
  value       = kubernetes_namespace.flux.metadata[0].name
}

output "git_repository_name" {
  description = "Name of the GitRepository resource managed by the module"
  value       = kubernetes_manifest.git_repository.manifest.metadata.name
}

output "kustomization_name" {
  description = "Name of the Kustomization resource managed by the module"
  value       = kubernetes_manifest.kustomization.manifest.metadata.name
}
