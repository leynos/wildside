//! Traefik Gateway Module
//!
//! Deploys Traefik as an ingress controller using Helm and creates a
//! cert-manager ClusterIssuer for ACME certificate management with
//! Cloudflare DNS01 validation.

locals {
  namespace           = trimspace(var.namespace)
  helm_release_name   = trimspace(var.helm_release_name)
  cluster_issuer_name = trimspace(var.cluster_issuer_name)
  dashboard_hostname  = var.dashboard_hostname == null ? null : trimspace(var.dashboard_hostname)
  helm_timeout        = var.helm_timeout
  helm_inline_values  = var.helm_values
  helm_value_files    = [for path in var.helm_values_files : trimspace(path) if trimspace(path) != ""]
  helm_values         = concat(local.helm_inline_values, [for path in local.helm_value_files : file(path)])

  # Construct default Helm values based on input variables.
  # These are merged with any user-provided values, with user values taking precedence.
  default_values = yamlencode({
    ingressClass = {
      enabled        = true
      isDefaultClass = var.ingress_class_default
      name           = var.ingress_class_name
    }
    service = merge(
      { type = var.service_type },
      var.service_type == "LoadBalancer" ? {
        spec = {
          externalTrafficPolicy = var.external_traffic_policy
        }
      } : {}
    )
    dashboard = {
      enabled = var.dashboard_enabled
    }
    ingressRoute = {
      dashboard = var.dashboard_enabled && local.dashboard_hostname != null ? {
        enabled   = true
        matchRule = format("Host(`%s`)", local.dashboard_hostname)
      } : {
        enabled = false
      }
    }
    ports = var.http_to_https_redirect ? {
      web = {
        redirectTo = {
          port = "websecure"
        }
      }
    } : {}
    metrics = {
      prometheus = {
        enabled = var.prometheus_metrics_enabled
        service = {
          enabled = var.prometheus_metrics_enabled
        }
        serviceMonitor = {
          enabled = var.prometheus_metrics_enabled && var.service_monitor_enabled
        }
      }
    }
    tolerations = var.tolerations
  })

  common_labels = {
    "app.kubernetes.io/managed-by" = "opentofu"
    "app.kubernetes.io/part-of"    = "traefik-gateway"
  }
}

resource "kubernetes_namespace" "traefik" {
  count = var.create_namespace ? 1 : 0

  metadata {
    name   = local.namespace
    labels = local.common_labels
  }
}

resource "helm_release" "traefik" {
  name       = local.helm_release_name
  repository = var.chart_repository
  chart      = var.chart_name
  version    = var.chart_version
  namespace  = var.create_namespace ? kubernetes_namespace.traefik[0].metadata[0].name : local.namespace
  timeout    = local.helm_timeout
  wait       = var.helm_wait
  atomic     = true

  create_namespace = false
  cleanup_on_fail  = true
  max_history      = 3

  values = concat([local.default_values], local.helm_values)

  depends_on = [kubernetes_namespace.traefik]
}

resource "kubernetes_manifest" "cluster_issuer" {
  manifest = {
    apiVersion = "cert-manager.io/v1"
    kind       = "ClusterIssuer"
    metadata = {
      name   = local.cluster_issuer_name
      labels = local.common_labels
    }
    spec = {
      acme = {
        server = var.acme_server
        email  = var.acme_email
        privateKeySecretRef = {
          name = local.cluster_issuer_name
        }
        solvers = [{
          dns01 = {
            cloudflare = {
              apiTokenSecretRef = {
                name = var.cloudflare_api_token_secret_name
                key  = var.cloudflare_api_token_secret_key
              }
            }
          }
        }]
      }
    }
  }
}
