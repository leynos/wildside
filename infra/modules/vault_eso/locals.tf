locals {
  mode           = trimspace(var.mode)
  is_apply_mode  = local.mode == "apply"
  is_render_mode = local.mode == "render"

  namespace = trimspace(var.namespace)
  effective_namespace = (
    local.is_apply_mode && var.create_namespace
    ? kubernetes_namespace.eso[0].metadata[0].name
    : local.namespace
  )
  helm_release_name = trimspace(var.helm_release_name)
  chart_repository  = trimspace(var.chart_repository)
  chart_name        = trimspace(var.chart_name)
  chart_version     = trimspace(var.chart_version)
  helm_timeout      = var.helm_timeout

  flux_namespace                = trimspace(var.flux_namespace)
  flux_helm_repository_name     = trimspace(var.flux_helm_repository_name)
  flux_helm_repository_interval = trimspace(var.flux_helm_repository_interval)
  flux_helm_release_interval    = trimspace(var.flux_helm_release_interval)
  crds_strategy                 = var.install_crds ? "CreateReplace" : "Skip"

  vault_address          = trimspace(var.vault_address)
  vault_ca_bundle_pem    = trimspace(var.vault_ca_bundle_pem)
  vault_ca_bundle_base64 = base64encode(local.vault_ca_bundle_pem)

  approle_role_id          = trimspace(var.approle_role_id)
  approle_secret_id        = trimspace(var.approle_secret_id)
  approle_auth_secret_name = trimspace(var.approle_auth_secret_name)
  approle_mount_path       = trimspace(var.approle_mount_path)

  kv_mount_path  = trimspace(var.kv_mount_path)
  pki_enabled    = var.pki_enabled
  pki_mount_path = trimspace(var.pki_mount_path)

  cluster_secret_store_kv_name    = trimspace(var.cluster_secret_store_kv_name)
  cluster_secret_store_pki_name   = trimspace(var.cluster_secret_store_pki_name)
  secret_store_retry_max_attempts = var.secret_store_retry_max_attempts
  secret_store_retry_interval     = trimspace(var.secret_store_retry_interval)

  helm_inline_values = [for value in var.helm_values : value if trimspace(value) != ""]
  helm_values        = local.helm_inline_values

  default_values_map = {
    installCRDs = var.install_crds
    webhook = {
      replicaCount = var.webhook_replica_count
      resources    = var.webhook_resources
    }
    resources = var.controller_resources
    certController = {
      resources = var.cert_controller_resources
    }
  }

  default_values_yaml    = yamlencode(local.default_values_map)
  decoded_helm_values    = [for value in local.helm_values : try(yamldecode(value), {})]
  merged_helm_values_map = merge({}, local.decoded_helm_values...)
  flux_values_map        = merge(local.default_values_map, local.merged_helm_values_map)

  common_labels = {
    "app.kubernetes.io/managed-by" = "opentofu"
    "app.kubernetes.io/part-of"    = "external-secrets"
  }

  webhook_pdb_enabled = var.pdb_enabled && var.webhook_replica_count > 1
}
