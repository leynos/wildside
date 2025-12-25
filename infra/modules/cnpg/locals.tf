locals {
  mode           = trimspace(var.mode)
  is_apply_mode  = local.mode == "apply"
  is_render_mode = local.mode == "render"

  # Namespace handling
  operator_namespace = trimspace(var.operator_namespace)
  cluster_namespace  = trimspace(var.cluster_namespace)
  effective_operator_namespace = (
    local.is_apply_mode && var.create_namespaces
    ? kubernetes_namespace.operator[0].metadata[0].name
    : local.operator_namespace
  )
  effective_cluster_namespace = (
    local.is_apply_mode && var.create_namespaces
    ? kubernetes_namespace.cluster[0].metadata[0].name
    : local.cluster_namespace
  )

  # Helm chart settings
  helm_release_name = trimspace(var.helm_release_name)
  chart_repository  = trimspace(var.chart_repository)
  chart_name        = trimspace(var.chart_name)
  chart_version     = trimspace(var.chart_version)
  helm_timeout      = var.helm_timeout

  # Flux settings
  flux_namespace                = trimspace(var.flux_namespace)
  flux_helm_repository_name     = trimspace(var.flux_helm_repository_name)
  flux_helm_repository_interval = trimspace(var.flux_helm_repository_interval)
  flux_helm_release_interval    = trimspace(var.flux_helm_release_interval)

  # Cluster settings
  cluster_name            = trimspace(var.cluster_name)
  instances               = var.instances
  image_name              = trimspace(var.image_name)
  storage_size            = trimspace(var.storage_size)
  storage_class           = trimspace(var.storage_class)
  database_name           = trimspace(var.database_name)
  database_owner          = trimspace(var.database_owner)
  primary_update_strategy = trimspace(var.primary_update_strategy)
  primary_update_method   = trimspace(var.primary_update_method)

  # Connection endpoints (CNPG naming convention)
  primary_service_name = "${local.cluster_name}-rw"
  replica_service_name = "${local.cluster_name}-ro"
  primary_endpoint     = "${local.primary_service_name}.${local.cluster_namespace}.svc.cluster.local"
  replica_endpoint     = "${local.replica_service_name}.${local.cluster_namespace}.svc.cluster.local"

  # PostGIS bootstrap SQL
  postgis_sql = var.postgis_enabled ? [
    "CREATE EXTENSION IF NOT EXISTS postgis;",
    "CREATE EXTENSION IF NOT EXISTS postgis_topology;"
  ] : []

  # Backup settings
  backup_enabled                    = var.backup_enabled
  backup_destination_path           = trimspace(var.backup_destination_path)
  backup_endpoint_url               = trimspace(var.backup_endpoint_url)
  backup_retention_policy           = trimspace(var.backup_retention_policy)
  backup_schedule                   = trimspace(var.backup_schedule)
  backup_s3_credentials_secret_name = trimspace(var.backup_s3_credentials_secret_name)
  wal_compression                   = trimspace(var.wal_compression)
  scheduled_backup_name             = trimspace(var.scheduled_backup_name)

  # ESO settings
  eso_enabled                       = var.eso_enabled
  eso_cluster_secret_store_name     = trimspace(var.eso_cluster_secret_store_name)
  eso_refresh_interval              = trimspace(var.eso_refresh_interval)
  superuser_credentials_vault_path  = trimspace(var.superuser_credentials_vault_path)
  superuser_credentials_secret_name = trimspace(var.superuser_credentials_secret_name)
  app_credentials_vault_path        = trimspace(var.app_credentials_vault_path)
  app_credentials_secret_name       = trimspace(var.app_credentials_secret_name)
  backup_credentials_vault_path     = trimspace(var.backup_credentials_vault_path)

  # Helm values processing
  helm_inline_values     = [for value in var.helm_values : value if trimspace(value) != ""]
  decoded_helm_values    = [for value in local.helm_inline_values : try(yamldecode(value), {})]
  merged_helm_values_map = merge({}, local.decoded_helm_values...)

  # PDB settings
  pdb_enabled = var.pdb_enabled && var.instances > 1
  pdb_name    = trimspace(var.pdb_name)

  # Common labels
  common_labels = {
    "app.kubernetes.io/managed-by" = "opentofu"
    "app.kubernetes.io/part-of"    = "cloudnative-pg"
  }

  # Cluster-specific labels
  cluster_labels = merge(local.common_labels, {
    "cnpg.io/cluster" = local.cluster_name
  })
}
