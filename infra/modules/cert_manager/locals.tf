locals {
  mode           = trimspace(var.mode)
  is_apply_mode  = local.mode == "apply"
  is_render_mode = local.mode == "render"

  namespace = trimspace(var.namespace)
  effective_namespace = (
    local.is_apply_mode && var.create_namespace ? kubernetes_namespace.cert_manager[0].metadata[0].name : local.namespace
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

  acme_email                              = trimspace(var.acme_email)
  acme_staging_enabled                    = var.acme_staging_enabled
  acme_production_enabled                 = var.acme_production_enabled
  acme_staging_server                     = trimspace(var.acme_staging_server)
  acme_production_server                  = trimspace(var.acme_production_server)
  acme_staging_issuer_name                = trimspace(var.acme_staging_issuer_name)
  acme_production_issuer_name             = trimspace(var.acme_production_issuer_name)
  acme_staging_account_key_secret_name    = trimspace(var.acme_staging_account_key_secret_name)
  acme_production_account_key_secret_name = trimspace(var.acme_production_account_key_secret_name)

  webhook_group_name  = trimspace(var.webhook_group_name)
  webhook_solver_name = trimspace(var.webhook_solver_name)
  namecheap_api_secret_name = (
    var.namecheap_api_secret_name == null ? null : trimspace(var.namecheap_api_secret_name)
  )
  namecheap_api_key_key  = trimspace(var.namecheap_api_key_key)
  namecheap_api_user_key = trimspace(var.namecheap_api_user_key)

  webhook_release_enabled      = var.webhook_release_enabled
  webhook_release_name         = trimspace(var.webhook_release_name)
  webhook_helm_repository_name = trimspace(var.webhook_helm_repository_name)
  webhook_repository_interval  = trimspace(var.webhook_repository_interval)
  webhook_release_interval     = trimspace(var.webhook_release_interval)
  webhook_chart_repository = (
    var.webhook_chart_repository == null ? null : trimspace(var.webhook_chart_repository)
  )
  webhook_chart_name    = trimspace(var.webhook_chart_name)
  webhook_chart_version = trimspace(var.webhook_chart_version)
  webhook_repository_type = (
    var.webhook_repository_type == null ? null : trimspace(var.webhook_repository_type)
  )
  webhook_release_replica_count = var.webhook_release_replica_count

  vault_enabled     = var.vault_enabled
  vault_issuer_name = trimspace(var.vault_issuer_name)
  vault_server      = var.vault_server == null ? null : trimspace(var.vault_server)
  vault_pki_path    = var.vault_pki_path == null ? null : trimspace(var.vault_pki_path)
  vault_token_secret_name = (
    var.vault_token_secret_name == null ? null : trimspace(var.vault_token_secret_name)
  )
  vault_token_secret_key = trimspace(var.vault_token_secret_key)
  vault_ca_bundle_pem = (
    var.vault_ca_bundle_pem == null ? null : trimspace(var.vault_ca_bundle_pem)
  )
  vault_ca_bundle_base64 = (
    local.vault_ca_bundle_pem != null ? base64encode(local.vault_ca_bundle_pem) : null
  )

  ca_bundle_secret_enabled = var.ca_bundle_secret_enabled
  ca_bundle_secret_name    = trimspace(var.ca_bundle_secret_name)
  ca_bundle_secret_key     = trimspace(var.ca_bundle_secret_key)

  helm_inline_values = [for value in var.helm_values : value if trimspace(value) != ""]
  helm_value_files = [
    for path in var.helm_values_files : trimspace(path) if trimspace(path) != ""
  ]
  helm_values = concat(
    local.helm_inline_values,
    [for path in local.helm_value_files : file(path)],
  )

  default_values_map = {
    installCRDs  = var.install_crds
    replicaCount = var.controller_replica_count
    resources    = var.controller_resources
    webhook = {
      replicaCount = var.webhook_replica_count
      resources    = var.webhook_resources
    }
    cainjector = {
      replicaCount = var.cainjector_replica_count
      resources    = var.cainjector_resources
    }
  }

  default_values_yaml    = yamlencode(local.default_values_map)
  decoded_helm_values    = [for value in local.helm_values : try(yamldecode(value), {})]
  merged_helm_values_map = merge({}, local.decoded_helm_values...)
  flux_values_map        = merge(local.default_values_map, local.merged_helm_values_map)

  common_labels = {
    "app.kubernetes.io/managed-by" = "opentofu"
    "app.kubernetes.io/part-of"    = "cert-manager"
  }

  acme_solver = {
    dns01 = {
      webhook = {
        groupName  = local.webhook_group_name
        solverName = local.webhook_solver_name
        config = {
          apiKeySecretRef = {
            name = local.namecheap_api_secret_name
            key  = local.namecheap_api_key_key
          }
          apiUserSecretRef = {
            name = local.namecheap_api_secret_name
            key  = local.namecheap_api_user_key
          }
        }
      }
    }
  }

  webhook_pdb_enabled    = var.pdb_enabled && var.webhook_replica_count > 1
  cainjector_pdb_enabled = var.pdb_enabled && var.cainjector_replica_count > 1
}
