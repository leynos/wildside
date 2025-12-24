resource "kubernetes_namespace" "eso" {
  count = local.is_apply_mode && var.create_namespace ? 1 : 0

  metadata {
    name   = local.namespace
    labels = local.common_labels
  }
}

resource "helm_release" "external_secrets" {
  count = local.is_apply_mode ? 1 : 0

  name             = local.helm_release_name
  namespace        = local.effective_namespace
  repository       = local.chart_repository
  chart            = local.chart_name
  version          = local.chart_version
  create_namespace = false
  wait             = var.helm_wait
  timeout          = local.helm_timeout

  values = concat([local.default_values_yaml], local.helm_values)

  depends_on = [kubernetes_namespace.eso]
}

resource "kubernetes_secret" "approle_auth" {
  count = local.is_apply_mode ? 1 : 0

  metadata {
    name      = local.approle_auth_secret_name
    namespace = local.effective_namespace
    labels    = local.common_labels
  }

  type = "Opaque"

  data = {
    role_id   = local.approle_role_id
    secret_id = local.approle_secret_id
  }

  depends_on = [kubernetes_namespace.eso]
}

resource "kubernetes_manifest" "cluster_secret_store_kv" {
  count = local.is_apply_mode ? 1 : 0

  manifest = {
    apiVersion = "external-secrets.io/v1beta1"
    kind       = "ClusterSecretStore"
    metadata = {
      name   = local.cluster_secret_store_kv_name
      labels = local.common_labels
    }
    spec = {
      provider = {
        vault = {
          server   = local.vault_address
          path     = local.kv_mount_path
          version  = "v2"
          caBundle = local.vault_ca_bundle_base64
          auth = {
            appRole = {
              path = local.approle_mount_path
              roleRef = {
                name      = local.approle_auth_secret_name
                namespace = local.effective_namespace
                key       = "role_id"
              }
              secretRef = {
                name      = local.approle_auth_secret_name
                namespace = local.effective_namespace
                key       = "secret_id"
              }
            }
          }
        }
      }
      retrySettings = {
        maxRetries    = local.secret_store_retry_max_attempts
        retryInterval = local.secret_store_retry_interval
      }
    }
  }

  depends_on = [
    helm_release.external_secrets,
    kubernetes_secret.approle_auth
  ]
}

check "pdb_min_available_constraint" {
  assert {
    condition = (
      !var.pdb_enabled ||
      var.webhook_replica_count <= 1 ||
      var.pdb_min_available < var.webhook_replica_count
    )
    error_message = "pdb_min_available (${var.pdb_min_available}) must be less than webhook_replica_count (${var.webhook_replica_count}) to allow rolling updates"
  }
}
