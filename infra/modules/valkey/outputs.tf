output "operator_namespace" {
  description = "Namespace where the Valkey operator is installed"
  value       = local.effective_operator_namespace
}

output "namespace" {
  description = "Namespace where the Valkey cluster runs"
  value       = local.effective_namespace
}

output "cluster_name" {
  description = "Name of the Valkey cluster resource"
  value       = local.cluster_name
}

output "helm_release_name" {
  description = "Name of the Valkey operator Helm release"
  value       = local.helm_release_name
}

output "primary_service_name" {
  description = "Kubernetes service name for the primary (read-write) instance"
  value       = local.primary_service_name
}

output "replica_service_name" {
  description = "Kubernetes service name for replica (read-only) instances"
  value       = local.replica_service_name
}

output "primary_endpoint" {
  description = "Fully qualified DNS endpoint for the primary (read-write) instance"
  value       = local.primary_endpoint
}

output "replica_endpoint" {
  description = "Fully qualified DNS endpoint for replica (read-only) instances"
  value       = local.replica_endpoint
}

output "port" {
  description = "Redis/Valkey port number"
  value       = local.valkey_port
}

output "credentials_secret_name" {
  description = "Kubernetes Secret name containing the Valkey password"
  value       = local.anonymous_auth ? null : local.password_secret_name
}

output "credentials_secret_key" {
  description = "Key within the credentials Secret that holds the password"
  value       = local.anonymous_auth ? null : local.password_secret_key
}

output "tls_enabled" {
  description = "Whether TLS is enabled for Valkey connections"
  value       = local.tls_enabled
}

output "sync_policy_contract" {
  description = "Contract for downstream workloads to consume Valkey cluster"
  value = {
    cluster = {
      name      = local.cluster_name
      namespace = local.effective_namespace
    }
    endpoints = {
      primary = {
        host = local.primary_endpoint
        port = local.valkey_port
      }
      replica = {
        host = local.replica_endpoint
        port = local.valkey_port
      }
    }
    credentials = local.anonymous_auth ? null : {
      secret_name = local.password_secret_name
      secret_key  = local.password_secret_key
      namespace   = local.effective_namespace
    }
    tls = {
      enabled     = local.tls_enabled
      cert_issuer = local.tls_enabled ? local.cert_issuer_name : null
    }
    persistence = {
      enabled       = var.persistence_enabled
      storage_class = local.storage_class
      size          = local.storage_size
    }
    replication = {
      nodes    = local.nodes
      replicas = local.replicas
    }
  }
}

output "rendered_manifests" {
  description = "Rendered Flux-ready manifests keyed by GitOps path (only populated when mode=render)"
  value       = local.is_render_mode ? local.rendered_manifests : {}
  sensitive   = true
}
