# Apply-mode resources
#
# These resources are only created when mode = "apply". In render mode,
# the module outputs Flux-ready manifests instead.

resource "kubernetes_namespace" "operator" {
  count = local.is_apply_mode && var.create_namespaces ? 1 : 0

  metadata {
    name   = local.operator_namespace
    labels = local.common_labels
  }
}

resource "kubernetes_namespace" "cluster" {
  count = local.is_apply_mode && var.create_namespaces ? 1 : 0

  metadata {
    name   = local.cluster_namespace
    labels = local.common_labels
  }
}

resource "helm_release" "cnpg_operator" {
  count = local.is_apply_mode ? 1 : 0

  name       = local.helm_release_name
  namespace  = local.effective_operator_namespace
  repository = local.chart_repository
  chart      = local.chart_name
  version    = local.chart_version
  wait       = var.helm_wait
  timeout    = local.helm_timeout

  dynamic "set" {
    for_each = local.merged_helm_values_map
    content {
      name  = set.key
      value = set.value
    }
  }

  depends_on = [kubernetes_namespace.operator]
}

resource "kubernetes_manifest" "cluster" {
  count = local.is_apply_mode ? 1 : 0

  manifest = local.cluster_manifest

  depends_on = [
    helm_release.cnpg_operator,
    kubernetes_namespace.cluster
  ]
}

resource "kubernetes_manifest" "scheduled_backup" {
  count = local.is_apply_mode && local.backup_enabled ? 1 : 0

  manifest = local.scheduled_backup_manifest

  depends_on = [kubernetes_manifest.cluster]
}

resource "kubernetes_secret" "s3_credentials" {
  count = (
    local.is_apply_mode &&
    local.backup_enabled &&
    length(trimspace(var.backup_s3_access_key_id)) > 0 &&
    length(trimspace(var.backup_s3_secret_access_key)) > 0
  ) ? 1 : 0

  metadata {
    name      = local.backup_s3_credentials_secret_name
    namespace = local.effective_cluster_namespace
    labels    = local.cluster_labels
  }

  type = "Opaque"

  data = {
    ACCESS_KEY_ID     = var.backup_s3_access_key_id
    SECRET_ACCESS_KEY = var.backup_s3_secret_access_key
  }

  depends_on = [kubernetes_namespace.cluster]
}

resource "kubernetes_manifest" "pdb" {
  count = local.is_apply_mode && local.pdb_enabled ? 1 : 0

  manifest = local.pdb_manifest

  depends_on = [kubernetes_namespace.cluster]
}

resource "kubernetes_manifest" "superuser_external_secret" {
  count = (
    local.is_apply_mode &&
    local.eso_enabled &&
    length(local.superuser_credentials_vault_path) > 0
  ) ? 1 : 0

  manifest = local.superuser_external_secret_manifest

  depends_on = [kubernetes_namespace.cluster]
}

resource "kubernetes_manifest" "app_external_secret" {
  count = (
    local.is_apply_mode &&
    local.eso_enabled &&
    length(local.app_credentials_vault_path) > 0
  ) ? 1 : 0

  manifest = local.app_external_secret_manifest

  depends_on = [kubernetes_namespace.cluster]
}

resource "kubernetes_manifest" "backup_external_secret" {
  count = (
    local.is_apply_mode &&
    local.eso_enabled &&
    local.backup_enabled &&
    length(local.backup_credentials_vault_path) > 0
  ) ? 1 : 0

  manifest = local.backup_external_secret_manifest

  depends_on = [kubernetes_namespace.cluster]
}

# Validation checks

check "pdb_min_available_constraint" {
  assert {
    condition = (
      !var.pdb_enabled ||
      var.instances <= 1 ||
      var.pdb_min_available < var.instances
    )
    error_message = "pdb_min_available (${var.pdb_min_available}) must be less than instances (${var.instances}) to allow rolling updates"
  }
}

check "backup_requires_destination" {
  assert {
    condition = (
      !var.backup_enabled ||
      length(trimspace(var.backup_destination_path)) > 0
    )
    error_message = "backup_destination_path is required when backup_enabled is true"
  }
}

check "backup_requires_endpoint" {
  assert {
    condition = (
      !var.backup_enabled ||
      length(trimspace(var.backup_endpoint_url)) > 0
    )
    error_message = "backup_endpoint_url is required when backup_enabled is true"
  }
}

check "backup_credentials_source" {
  assert {
    condition = (
      !var.backup_enabled ||
      (length(trimspace(var.backup_s3_access_key_id)) > 0 && length(trimspace(var.backup_s3_secret_access_key)) > 0) ||
      (var.eso_enabled && length(trimspace(var.backup_credentials_vault_path)) > 0)
    )
    error_message = "When backup_enabled is true, provide either inline S3 credentials or a backup_credentials_vault_path with eso_enabled"
  }
}

check "eso_superuser_path_required" {
  assert {
    condition = (
      !var.eso_enabled ||
      length(trimspace(var.superuser_credentials_vault_path)) > 0
    )
    error_message = "superuser_credentials_vault_path is required when eso_enabled is true"
  }
}
