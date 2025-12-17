//! ExternalDNS Module
//!
//! Deploys ExternalDNS as a DNS automation controller using Helm. ExternalDNS
//! watches Kubernetes resources (Ingress, Service) and automatically manages
//! DNS records in Cloudflare based on annotations.

locals {
  mode           = trimspace(var.mode)
  is_apply_mode  = local.mode == "apply"
  is_render_mode = local.mode == "render"

  namespace                        = trimspace(var.namespace)
  helm_release_name                = trimspace(var.helm_release_name)
  chart_repository                 = trimspace(var.chart_repository)
  chart_name                       = trimspace(var.chart_name)
  chart_version                    = trimspace(var.chart_version)
  cloudflare_api_token_secret_name = trimspace(var.cloudflare_api_token_secret_name)
  cloudflare_api_token_secret_key  = trimspace(var.cloudflare_api_token_secret_key)
  txt_owner_id                     = trimspace(var.txt_owner_id)
  policy                           = trimspace(var.policy)
  log_level                        = trimspace(var.log_level)
  interval                         = trimspace(var.interval)
  registry_type                    = trimspace(var.registry_type)
  helm_timeout                     = var.helm_timeout

  domain_filters = [for domain in var.domain_filters : lower(trimspace(domain))]
  sources        = [for source in var.sources : trimspace(source)]

  helm_inline_values = [for value in var.helm_values : value if trimspace(value) != ""]
  helm_value_files = [
    for path in var.helm_values_files : trimspace(path) if trimspace(path) != ""
  ]
  helm_values = concat(
    local.helm_inline_values,
    [for path in local.helm_value_files : file(path)],
  )

  flux_namespace            = trimspace(var.flux_namespace)
  flux_helm_repository_name = trimspace(var.flux_helm_repository_name)

  # Construct extra arguments for ExternalDNS
  extra_args = compact(concat(
    var.cloudflare_proxied ? ["--cloudflare-proxied"] : [],
    ["--cloudflare-dns-records-per-page=${var.dns_records_per_page}"],
    var.txt_prefix != "" ? ["--txt-prefix=${var.txt_prefix}"] : [],
    var.txt_suffix != "" ? ["--txt-suffix=${var.txt_suffix}"] : [],
  ))

  # Construct default Helm values based on input variables.
  # These are merged with any user-provided values, with user values taking precedence.
  default_values_map = {
    provider = {
      name = "cloudflare"
    }
    env = [
      {
        name = "CF_API_TOKEN"
        valueFrom = {
          secretKeyRef = {
            name = local.cloudflare_api_token_secret_name
            key  = local.cloudflare_api_token_secret_key
          }
        }
      }
    ]
    domainFilters = local.domain_filters
    policy        = local.policy
    txtOwnerId    = local.txt_owner_id
    sources       = local.sources
    interval      = local.interval
    registry      = local.registry_type
    logLevel      = local.log_level
    extraArgs     = local.extra_args
    crd = {
      create = var.crd_enabled
    }
    serviceMonitor = {
      enabled = var.service_monitor_enabled
    }
  }

  default_values_yaml = yamlencode(local.default_values_map)

  decoded_helm_values = [for value in local.helm_values : yamldecode(value)]

  merged_helm_values_map = merge({}, local.decoded_helm_values...)

  flux_values_map = merge(local.default_values_map, local.merged_helm_values_map)

  common_labels = {
    "app.kubernetes.io/managed-by" = "opentofu"
    "app.kubernetes.io/part-of"    = "external-dns"
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

  external_dns_namespace_manifest = {
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

  kustomization_manifest = {
    apiVersion = "kustomize.config.k8s.io/v1beta1"
    kind       = "Kustomization"
    resources = [
      "namespace.yaml",
      "helmrelease.yaml",
    ]
  }

  rendered_manifests = {
    "platform/sources/external-dns-repo.yaml"  = format("%s\n", yamlencode(local.flux_helm_repository_manifest))
    "platform/external-dns/namespace.yaml"     = format("%s\n", yamlencode(local.external_dns_namespace_manifest))
    "platform/external-dns/helmrelease.yaml"   = format("%s\n", yamlencode(local.flux_helmrelease_manifest))
    "platform/external-dns/kustomization.yaml" = format("%s\n", yamlencode(local.kustomization_manifest))
  }
}

resource "kubernetes_namespace" "external_dns" {
  count = local.is_apply_mode && var.create_namespace ? 1 : 0

  metadata {
    name   = local.namespace
    labels = local.common_labels
  }
}

resource "helm_release" "external_dns" {
  count      = local.is_apply_mode ? 1 : 0
  name       = local.helm_release_name
  repository = local.chart_repository
  chart      = local.chart_name
  version    = local.chart_version
  namespace = (
    local.is_apply_mode && var.create_namespace ? kubernetes_namespace.external_dns[0].metadata[0].name : local.namespace
  )
  timeout = local.helm_timeout
  wait    = var.helm_wait
  atomic  = true

  create_namespace = false
  cleanup_on_fail  = true
  max_history      = 3

  values = concat([local.default_values_yaml], local.helm_values)
}
