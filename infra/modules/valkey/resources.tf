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
    name   = local.namespace
    labels = local.common_labels
  }
}

resource "helm_release" "valkey_operator" {
  count = local.is_apply_mode ? 1 : 0

  name       = local.helm_release_name
  namespace  = local.effective_operator_namespace
  repository = local.chart_repository
  chart      = local.chart_name
  version    = local.chart_version
  wait       = var.helm_wait
  timeout    = local.helm_timeout

  # Helm provider 3.x uses list of objects instead of dynamic blocks
  set = [for k, v in local.merged_helm_values_map : { name = k, value = v }]

  depends_on = [kubernetes_namespace.operator]
}

resource "kubernetes_manifest" "valkey_cluster" {
  count = local.is_apply_mode ? 1 : 0

  manifest = local.valkey_cluster_manifest

  depends_on = [
    helm_release.valkey_operator,
    kubernetes_namespace.cluster
  ]
}

resource "kubernetes_secret" "password" {
  count = local.is_apply_mode && local.create_password_secret ? 1 : 0

  metadata {
    name      = local.password_secret_name
    namespace = local.effective_namespace
    labels    = local.cluster_labels
  }

  type = "Opaque"

  data = {
    (local.password_secret_key) = var.password_inline
  }

  depends_on = [kubernetes_namespace.cluster]
}

resource "kubernetes_manifest" "pdb" {
  count = local.is_apply_mode && local.pdb_enabled ? 1 : 0

  manifest = local.pdb_manifest

  depends_on = [kubernetes_namespace.cluster]
}

resource "kubernetes_manifest" "password_external_secret" {
  count = (
    local.is_apply_mode &&
    local.eso_enabled &&
    !local.anonymous_auth &&
    length(local.password_vault_path) > 0
  ) ? 1 : 0

  manifest = local.password_external_secret_manifest

  depends_on = [kubernetes_namespace.cluster]
}

# Validation checks

check "pdb_min_available_constraint" {
  assert {
    condition = (
      !var.pdb_enabled ||
      var.replicas == 0 ||
      var.pdb_min_available < (var.nodes * (var.replicas + 1))
    )
    error_message = "pdb_min_available must be less than total pods (nodes * (replicas + 1)) to allow rolling updates"
  }
}

check "tls_requires_issuer" {
  assert {
    condition = (
      !var.tls_enabled ||
      length(trimspace(var.cert_issuer_name)) > 0
    )
    error_message = "cert_issuer_name is required when tls_enabled is true"
  }
}

check "auth_requires_password_source" {
  assert {
    condition = (
      var.anonymous_auth ||
      (var.eso_enabled && length(trimspace(var.password_vault_path)) > 0) ||
      length(trimspace(var.password_inline)) > 0
    )
    error_message = "When anonymous_auth is false, provide either password_inline or password_vault_path with eso_enabled"
  }
}

check "eso_password_path_required" {
  assert {
    condition = (
      !var.eso_enabled ||
      var.anonymous_auth ||
      length(trimspace(var.password_vault_path)) > 0
    )
    error_message = "password_vault_path is required when eso_enabled is true and anonymous_auth is false"
  }
}
