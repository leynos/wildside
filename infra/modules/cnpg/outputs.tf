output "operator_namespace" {
  description = "Namespace where CloudNativePG operator is installed"
  value       = local.effective_operator_namespace
}

output "cluster_namespace" {
  description = "Namespace where the PostgreSQL cluster runs"
  value       = local.effective_cluster_namespace
}

output "cluster_name" {
  description = "Name of the CNPG Cluster resource"
  value       = local.cluster_name
}

output "helm_release_name" {
  description = "Name of the CloudNativePG operator Helm release"
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

output "database_name" {
  description = "Name of the initial database"
  value       = local.database_name
}

output "database_owner" {
  description = "Owner username for the initial database"
  value       = local.database_owner
}

output "superuser_credentials_secret_name" {
  description = "Kubernetes Secret name containing superuser credentials"
  value       = local.superuser_credentials_secret_name
}

output "app_credentials_secret_name" {
  description = "Kubernetes Secret name containing application credentials"
  value       = local.app_credentials_secret_name
}

output "backup_enabled" {
  description = "Whether backups are enabled for the cluster"
  value       = local.backup_enabled
}

output "sync_policy_contract" {
  description = "Contract for downstream workloads to consume PostgreSQL cluster"
  value = {
    cluster = {
      name      = local.cluster_name
      namespace = local.cluster_namespace
    }
    endpoints = {
      primary = {
        host = local.primary_endpoint
        port = 5432
      }
      replica = {
        host = local.replica_endpoint
        port = 5432
      }
    }
    database = {
      name  = local.database_name
      owner = local.database_owner
    }
    credentials = {
      superuser_secret = {
        name      = local.superuser_credentials_secret_name
        namespace = local.cluster_namespace
      }
      app_secret = {
        name      = local.app_credentials_secret_name
        namespace = local.cluster_namespace
      }
    }
    backup = local.backup_enabled ? {
      enabled          = true
      destination_path = local.backup_destination_path
      schedule         = local.backup_schedule
    } : null
    postgis_enabled = var.postgis_enabled
  }
}

output "rendered_manifests" {
  description = "Rendered Flux-ready manifests keyed by GitOps path (only populated when mode=render)"
  value       = local.is_render_mode ? local.rendered_manifests : {}
  sensitive   = true
}
