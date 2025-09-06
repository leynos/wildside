output "cluster_id" {
  description = "Identifier of the Kubernetes cluster"
  value       = digitalocean_kubernetes_cluster.this.id
}

output "endpoint" {
  description = "API server endpoint"
  value       = digitalocean_kubernetes_cluster.this.endpoint
}

output "kubeconfig" {
  description = "Raw kubeconfig for the cluster"
  sensitive   = true
  value       = var.expose_kubeconfig && length(digitalocean_kubernetes_cluster.this.kube_config) > 0 ? digitalocean_kubernetes_cluster.this.kube_config[0].raw_config : null
}
