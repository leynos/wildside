locals {
  mode           = trimspace(var.mode)
  is_apply_mode  = local.mode == "apply"
  is_render_mode = local.mode == "render"

  # Namespace handling
  namespace          = trimspace(var.namespace)
  operator_namespace = trimspace(var.operator_namespace)
  effective_namespace = (
    local.is_apply_mode && var.create_namespaces
    ? kubernetes_namespace.cluster[0].metadata[0].name
    : local.namespace
  )
  effective_operator_namespace = (
    local.is_apply_mode && var.create_namespaces
    ? kubernetes_namespace.operator[0].metadata[0].name
    : local.operator_namespace
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
  cluster_name   = trimspace(var.cluster_name)
  cluster_domain = trimspace(var.cluster_domain)
  nodes          = var.nodes
  replicas       = var.replicas
  storage_size   = trimspace(var.storage_size)
  storage_class  = trimspace(var.storage_class)

  # Connection endpoints (Valkey service naming convention)
  # The Valkey operator creates services named <cluster>-primary and <cluster>-replicas
  primary_service_name = "${local.cluster_name}-primary"
  replica_service_name = "${local.cluster_name}-replicas"
  primary_endpoint     = "${local.primary_service_name}.${local.namespace}.svc.${local.cluster_domain}"
  replica_endpoint     = "${local.replica_service_name}.${local.namespace}.svc.${local.cluster_domain}"
  valkey_port          = 6379

  # Credentials settings
  anonymous_auth       = var.anonymous_auth
  password_secret_name = trimspace(var.password_secret_name)
  password_secret_key  = trimspace(var.password_secret_key)

  # ESO settings
  eso_enabled                   = var.eso_enabled
  eso_cluster_secret_store_name = trimspace(var.eso_cluster_secret_store_name)
  eso_refresh_interval          = trimspace(var.eso_refresh_interval)
  password_vault_path           = trimspace(var.password_vault_path)
  password_vault_key            = trimspace(var.password_vault_key)

  # TLS settings
  tls_enabled       = var.tls_enabled
  cert_issuer_name  = trimspace(var.cert_issuer_name)
  cert_issuer_type  = var.cert_issuer_type

  # Helm values processing
  helm_inline_values     = [for value in var.helm_values : value if trimspace(value) != ""]
  decoded_helm_values    = [for value in local.helm_inline_values : try(yamldecode(value), {})]
  merged_helm_values_map = merge({}, local.decoded_helm_values...)

  # PDB settings - only enable if replicas > 0 (HA mode)
  pdb_enabled = var.pdb_enabled && var.replicas > 0
  pdb_name    = trimspace(var.pdb_name)

  # Common labels
  common_labels = {
    "app.kubernetes.io/managed-by" = "opentofu"
    "app.kubernetes.io/part-of"    = "valkey"
  }

  # Cluster-specific labels
  cluster_labels = merge(local.common_labels, {
    "valkey.hyperspike.io/cluster" = local.cluster_name
  })

  # Determine if we need to create a password secret
  create_password_secret = (
    !local.anonymous_auth &&
    !local.eso_enabled &&
    length(trimspace(var.password_inline)) > 0
  )
}
