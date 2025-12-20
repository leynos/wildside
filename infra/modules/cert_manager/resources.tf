resource "kubernetes_namespace" "cert_manager" {
  count = local.is_apply_mode && var.create_namespace ? 1 : 0

  metadata {
    name   = local.namespace
    labels = local.common_labels
  }
}

resource "helm_release" "cert_manager" {
  count      = local.is_apply_mode ? 1 : 0
  name       = local.helm_release_name
  repository = local.chart_repository
  chart      = local.chart_name
  version    = local.chart_version
  namespace  = local.effective_namespace
  timeout    = local.helm_timeout
  wait       = var.helm_wait
  atomic     = true
  depends_on = [kubernetes_namespace.cert_manager]

  create_namespace = false
  cleanup_on_fail  = true
  max_history      = 3

  values = concat([local.default_values_yaml], local.helm_values)
}

resource "helm_release" "namecheap_webhook" {
  count      = local.is_apply_mode && local.webhook_release_enabled ? 1 : 0
  name       = local.webhook_release_name
  repository = local.webhook_chart_repository
  chart      = local.webhook_chart_name
  version    = local.webhook_chart_version
  namespace  = local.effective_namespace
  timeout    = local.helm_timeout
  wait       = var.helm_wait
  atomic     = true
  depends_on = [helm_release.cert_manager]

  create_namespace = false
  cleanup_on_fail  = true
  max_history      = 3

  values = [yamlencode({
    replicaCount = local.webhook_release_replica_count
    groupName    = local.webhook_group_name
  })]
}

resource "kubernetes_manifest" "acme_staging_issuer" {
  count    = local.is_apply_mode && local.acme_staging_enabled ? 1 : 0
  manifest = local.acme_staging_issuer_manifest
  depends_on = [
    helm_release.cert_manager,
    helm_release.namecheap_webhook,
  ]
}

resource "kubernetes_manifest" "acme_production_issuer" {
  count    = local.is_apply_mode && local.acme_production_enabled ? 1 : 0
  manifest = local.acme_production_issuer_manifest
  depends_on = [
    helm_release.cert_manager,
    helm_release.namecheap_webhook,
  ]
}

resource "kubernetes_manifest" "vault_issuer" {
  count    = local.is_apply_mode && local.vault_enabled ? 1 : 0
  manifest = local.vault_issuer_manifest
  depends_on = [helm_release.cert_manager]
}

resource "kubernetes_manifest" "webhook_pdb" {
  count    = local.is_apply_mode && local.webhook_pdb_enabled ? 1 : 0
  manifest = local.webhook_pdb_manifest
  depends_on = [helm_release.cert_manager]
}

resource "kubernetes_manifest" "cainjector_pdb" {
  count    = local.is_apply_mode && local.cainjector_pdb_enabled ? 1 : 0
  manifest = local.cainjector_pdb_manifest
  depends_on = [helm_release.cert_manager]
}

resource "kubernetes_manifest" "ca_bundle_secret" {
  count    = local.is_apply_mode && local.ca_bundle_secret_enabled ? 1 : 0
  manifest = local.ca_bundle_secret_manifest
  depends_on = [helm_release.cert_manager]
}
