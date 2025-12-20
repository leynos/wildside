# Traefik Gateway Module
#
# Deploys Traefik as an ingress controller using Helm and creates a
# cert-manager ClusterIssuer for ACME certificate management with
# Cloudflare DNS01 validation.

locals {
  mode           = trimspace(var.mode)
  is_apply_mode  = local.mode == "apply"
  is_render_mode = local.mode == "render"

  namespace           = trimspace(var.namespace)
  helm_release_name   = trimspace(var.helm_release_name)
  cluster_issuer_name = trimspace(var.cluster_issuer_name)
  dashboard_hostname  = var.dashboard_hostname == null ? null : trimspace(var.dashboard_hostname)
  chart_repository    = trimspace(var.chart_repository)
  chart_name          = trimspace(var.chart_name)
  chart_version       = trimspace(var.chart_version)
  service_type        = trimspace(var.service_type)
  service_annotations = { for key, value in var.service_annotations : trimspace(key) => trimspace(value) }
  external_traffic_policy = (
    var.external_traffic_policy == null ? null : trimspace(var.external_traffic_policy)
  )
  ingress_class_name               = trimspace(var.ingress_class_name)
  acme_email                       = trimspace(var.acme_email)
  acme_server                      = trimspace(var.acme_server)
  cloudflare_api_token_secret_name = trimspace(var.cloudflare_api_token_secret_name)
  cloudflare_api_token_secret_key  = trimspace(var.cloudflare_api_token_secret_key)
  helm_timeout                     = var.helm_timeout
  helm_inline_values               = [for value in var.helm_values : value if trimspace(value) != ""]
  helm_value_files = [
    for path in var.helm_values_files : trimspace(path) if trimspace(path) != ""
  ]
  helm_values = concat(
    local.helm_inline_values,
    [for path in local.helm_value_files : file(path)],
  )

  flux_namespace            = trimspace(var.flux_namespace)
  flux_helm_repository_name = trimspace(var.flux_helm_repository_name)

  crds_dir  = "${path.module}/crds"
  crd_files = fileset(local.crds_dir, "*.yaml")

  # Construct default Helm values based on input variables.
  # These are merged with any user-provided values, with user values taking precedence.
  default_values_map = {
    ingressClass = {
      enabled        = true
      isDefaultClass = var.ingress_class_default
      name           = local.ingress_class_name
    }
    service = merge({
      type        = local.service_type
      annotations = local.service_annotations
      }, local.service_type == "LoadBalancer" ? {
      spec = {
        externalTrafficPolicy = local.external_traffic_policy
      }
    } : {})
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
  }

  default_values_yaml = yamlencode(local.default_values_map)

  decoded_helm_values = [for value in local.helm_values : yamldecode(value)]

  merged_helm_values_map = merge({}, local.decoded_helm_values...)

  flux_values_map = merge(local.default_values_map, local.merged_helm_values_map)

  common_labels = {
    "app.kubernetes.io/managed-by" = "opentofu"
    "app.kubernetes.io/part-of"    = "traefik-gateway"
  }

  flux_helm_repository_manifest = {
    apiVersion = "source.toolkit.fluxcd.io/v1"
    kind       = "HelmRepository"
    metadata = {
      name      = local.flux_helm_repository_name
      namespace = local.flux_namespace
      labels    = local.common_labels
    }
    spec = {
      interval = "1h"
      url      = local.chart_repository
    }
  }

  traefik_namespace_manifest = {
    apiVersion = "v1"
    kind       = "Namespace"
    metadata = {
      name   = local.namespace
      labels = local.common_labels
    }
  }

  flux_helmrelease_manifest = {
    apiVersion = "helm.toolkit.fluxcd.io/v2"
    kind       = "HelmRelease"
    metadata = {
      name      = local.helm_release_name
      namespace = local.namespace
      labels    = local.common_labels
    }
    spec = {
      interval = "30m"
      install = {
        crds = "CreateReplace"
        remediation = {
          retries = 3
        }
      }
      upgrade = {
        crds = "CreateReplace"
        remediation = {
          retries              = 3
          remediateLastFailure = true
        }
      }
      chart = {
        spec = {
          chart = local.chart_name
          sourceRef = {
            kind      = "HelmRepository"
            name      = local.flux_helm_repository_name
            namespace = local.flux_namespace
          }
          version = local.chart_version
        }
      }
      values = local.flux_values_map
    }
  }

  cluster_issuer_manifest = {
    apiVersion = "cert-manager.io/v1"
    kind       = "ClusterIssuer"
    metadata = {
      name   = local.cluster_issuer_name
      labels = local.common_labels
    }
    spec = {
      acme = {
        server = local.acme_server
        email  = local.acme_email
        privateKeySecretRef = {
          name = local.cluster_issuer_name
        }
        solvers = [{
          dns01 = {
            cloudflare = {
              apiTokenSecretRef = {
                name = local.cloudflare_api_token_secret_name
                key  = local.cloudflare_api_token_secret_key
              }
            }
          }
        }]
      }
    }
  }

  kustomization_manifest = {
    apiVersion = "kustomize.config.k8s.io/v1beta1"
    kind       = "Kustomization"
    resources = concat(
      ["namespace.yaml"],
      ["cluster-issuer.yaml"],
      ["crds/traefik-crds.yaml"],
      ["helmrelease.yaml"],
    )
  }

  rendered_crds = {
    for filename in local.crd_files :
    "platform/traefik/crds/${filename}" => format("%s\n", file("${local.crds_dir}/${filename}"))
  }

  rendered_manifests = merge(
    local.rendered_crds,
    {
      "platform/sources/traefik-repo.yaml"   = format("%s\n", yamlencode(local.flux_helm_repository_manifest))
      "platform/traefik/namespace.yaml"      = format("%s\n", yamlencode(local.traefik_namespace_manifest))
      "platform/traefik/helmrelease.yaml"    = format("%s\n", yamlencode(local.flux_helmrelease_manifest))
      "platform/traefik/cluster-issuer.yaml" = format("%s\n", yamlencode(local.cluster_issuer_manifest))
      "platform/traefik/kustomization.yaml"  = format("%s\n", yamlencode(local.kustomization_manifest))
    },
  )
}

resource "kubernetes_namespace" "traefik" {
  count = local.is_apply_mode && var.create_namespace ? 1 : 0

  metadata {
    name   = local.namespace
    labels = local.common_labels
  }
}

moved {
  from = helm_release.traefik
  to   = helm_release.traefik[0]
}

resource "helm_release" "traefik" {
  count      = local.is_apply_mode ? 1 : 0
  name       = local.helm_release_name
  repository = local.chart_repository
  chart      = local.chart_name
  version    = local.chart_version
  namespace = (
    local.is_apply_mode && var.create_namespace ? kubernetes_namespace.traefik[0].metadata[0].name : local.namespace
  )
  timeout = local.helm_timeout
  wait    = var.helm_wait
  atomic  = true

  create_namespace = false
  cleanup_on_fail  = true
  max_history      = 3

  values = concat([local.default_values_yaml], local.helm_values)
}

moved {
  from = kubernetes_manifest.cluster_issuer
  to   = kubernetes_manifest.cluster_issuer[0]
}

resource "kubernetes_manifest" "cluster_issuer" {
  count = local.is_apply_mode ? 1 : 0
  manifest = {
    apiVersion = "cert-manager.io/v1"
    kind       = "ClusterIssuer"
    metadata = {
      name   = local.cluster_issuer_name
      labels = local.common_labels
    }
    spec = {
      acme = {
        server = local.acme_server
        email  = local.acme_email
        privateKeySecretRef = {
          name = local.cluster_issuer_name
        }
        solvers = [{
          dns01 = {
            cloudflare = {
              apiTokenSecretRef = {
                name = local.cloudflare_api_token_secret_name
                key  = local.cloudflare_api_token_secret_key
              }
            }
          }
        }]
      }
    }
  }
}
