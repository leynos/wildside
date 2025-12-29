locals {
  # HelmRepository for Valkey operator chart
  flux_helm_repository_manifest = {
    apiVersion = "source.toolkit.fluxcd.io/v1"
    kind       = "HelmRepository"
    metadata = {
      name      = local.flux_helm_repository_name
      namespace = local.flux_namespace
      labels    = local.common_labels
    }
    spec = merge({
      interval = local.flux_helm_repository_interval
      url      = local.chart_repository
      }, startswith(local.chart_repository, "oci://") ? {
      type = "oci"
    } : {})
  }

  # Operator namespace
  operator_namespace_manifest = {
    apiVersion = "v1"
    kind       = "Namespace"
    metadata = {
      name   = local.operator_namespace
      labels = local.common_labels
    }
  }

  # Cluster namespace
  cluster_namespace_manifest = {
    apiVersion = "v1"
    kind       = "Namespace"
    metadata = {
      name   = local.namespace
      labels = local.common_labels
    }
  }

  # Operator HelmRelease
  flux_helmrelease_manifest = {
    apiVersion = "helm.toolkit.fluxcd.io/v2"
    kind       = "HelmRelease"
    metadata = {
      name      = local.helm_release_name
      namespace = local.operator_namespace
      labels    = local.common_labels
    }
    spec = {
      interval = local.flux_helm_release_interval
      install = {
        remediation = {
          retries = 3
        }
      }
      upgrade = {
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
      values = local.merged_helm_values_map
    }
  }

  # Valkey Cluster resource
  valkey_cluster_manifest = {
    apiVersion = "hyperspike.io/v1"
    kind       = "Valkey"
    metadata = {
      name      = local.cluster_name
      namespace = local.namespace
      labels    = local.cluster_labels
    }
    spec = merge(
      {
        nodes             = local.nodes
        replicas          = local.replicas
        anonymousAuth     = local.anonymous_auth
        clusterDomain     = local.cluster_domain
        prometheus        = var.prometheus_enabled
        serviceMonitor    = var.service_monitor_enabled
        volumePermissions = false
        tls               = local.tls_enabled && length(local.cert_issuer_name) > 0
      },
      local.tls_enabled && length(local.cert_issuer_name) > 0 ? {
        certIssuer     = local.cert_issuer_name
        certIssuerType = local.cert_issuer_type
      } : {},
      var.persistence_enabled ? {
        storage = {
          resources = {
            requests = {
              storage = local.storage_size
            }
          }
          storageClassName = local.storage_class
        }
      } : {},
      !local.anonymous_auth ? {
        servicePassword = {
          name = local.password_secret_name
          key  = local.password_secret_key
        }
      } : {},
      length(trimspace(var.image)) > 0 ? {
        image = trimspace(var.image)
      } : {},
      length(trimspace(var.exporter_image)) > 0 ? {
        exporterImage = trimspace(var.exporter_image)
      } : {},
      {
        resources = {
          requests = {
            cpu    = var.resource_requests.cpu
            memory = var.resource_requests.memory
          }
          limits = {
            cpu    = var.resource_limits.cpu
            memory = var.resource_limits.memory
          }
        }
      },
      length(var.node_selector) > 0 ? {
        nodeSelector = var.node_selector
      } : {},
      length(var.tolerations) > 0 ? {
        tolerations = var.tolerations
      } : {},
      var.external_access_enabled ? {
        externalAccess = {
          type = var.external_access_type
        }
      } : {}
    )
  }

  # PodDisruptionBudget for cluster (when replicas > 0)
  pdb_manifest = local.pdb_enabled ? {
    apiVersion = "policy/v1"
    kind       = "PodDisruptionBudget"
    metadata = {
      name      = local.pdb_name
      namespace = local.namespace
      labels    = local.cluster_labels
    }
    spec = {
      minAvailable = var.pdb_min_available
      selector = {
        matchLabels = {
          "valkey.hyperspike.io/cluster" = local.cluster_name
        }
      }
    }
  } : null

  # Password Secret (when inline password provided and ESO disabled)
  password_secret_manifest = local.create_password_secret ? {
    apiVersion = "v1"
    kind       = "Secret"
    metadata = {
      name      = local.password_secret_name
      namespace = local.namespace
      labels    = local.cluster_labels
    }
    type = "Opaque"
    stringData = {
      (local.password_secret_key) = var.password_inline
    }
  } : null

  # ExternalSecret for password (when ESO enabled)
  password_external_secret_manifest = (
    local.eso_enabled &&
    !local.anonymous_auth &&
    length(local.password_vault_path) > 0
    ) ? {
    apiVersion = "external-secrets.io/v1"
    kind       = "ExternalSecret"
    metadata = {
      name      = local.password_secret_name
      namespace = local.namespace
      labels    = local.cluster_labels
    }
    spec = {
      refreshInterval = local.eso_refresh_interval
      secretStoreRef = {
        name = local.eso_cluster_secret_store_name
        kind = "ClusterSecretStore"
      }
      target = {
        name           = local.password_secret_name
        creationPolicy = "Owner"
      }
      data = [
        {
          secretKey = local.password_secret_key
          remoteRef = {
            key      = local.password_vault_path
            property = local.password_vault_key
          }
        }
      ]
    }
  } : null

  # Kustomization manifest
  kustomization_resources = concat(
    ["../sources/valkey-operator-repo.yaml"],
    ["namespace-valkey-system.yaml", "namespace-valkey.yaml"],
    ["valkey-operator-helmrelease.yaml"],
    ["valkey-cluster.yaml"],
    local.pdb_enabled ? ["pdb-valkey.yaml"] : [],
    local.password_secret_manifest != null ? ["password-secret.yaml"] : [],
    local.password_external_secret_manifest != null ? ["external-secret-password.yaml"] : []
  )

  kustomization_manifest = {
    apiVersion = "kustomize.config.k8s.io/v1beta1"
    kind       = "Kustomization"
    resources  = local.kustomization_resources
  }

  # Rendered manifests map
  rendered_manifests = merge(
    {
      "platform/sources/valkey-operator-repo.yaml" = format(
        "%s\n",
        yamlencode(local.flux_helm_repository_manifest)
      )
      "platform/redis/namespace-valkey-system.yaml" = format(
        "%s\n",
        yamlencode(local.operator_namespace_manifest)
      )
      "platform/redis/namespace-valkey.yaml" = format(
        "%s\n",
        yamlencode(local.cluster_namespace_manifest)
      )
      "platform/redis/valkey-operator-helmrelease.yaml" = format(
        "%s\n",
        yamlencode(local.flux_helmrelease_manifest)
      )
      "platform/redis/valkey-cluster.yaml" = format(
        "%s\n",
        yamlencode(local.valkey_cluster_manifest)
      )
      "platform/redis/kustomization.yaml" = format(
        "%s\n",
        yamlencode(local.kustomization_manifest)
      )
    },
    local.pdb_enabled ? {
      "platform/redis/pdb-valkey.yaml" = format(
        "%s\n",
        yamlencode(local.pdb_manifest)
      )
    } : {},
    local.password_secret_manifest != null ? {
      "platform/redis/password-secret.yaml" = format(
        "%s\n",
        yamlencode(local.password_secret_manifest)
      )
    } : {},
    local.password_external_secret_manifest != null ? {
      "platform/redis/external-secret-password.yaml" = format(
        "%s\n",
        yamlencode(local.password_external_secret_manifest)
      )
    } : {}
  )
}
