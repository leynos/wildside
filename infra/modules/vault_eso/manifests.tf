locals {
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

  eso_namespace_manifest = {
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
      interval = local.flux_helm_release_interval
      install = {
        crds = local.crds_strategy
        remediation = {
          retries = 3
        }
      }
      upgrade = {
        crds = local.crds_strategy
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

  approle_auth_secret_manifest = {
    apiVersion = "v1"
    kind       = "Secret"
    metadata = {
      name      = local.approle_auth_secret_name
      namespace = local.namespace
      labels    = local.common_labels
    }
    type = "Opaque"
    stringData = {
      role_id   = local.approle_role_id
      secret_id = local.approle_secret_id
    }
  }

  cluster_secret_store_kv_manifest = {
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
                namespace = local.namespace
                key       = "role_id"
              }
              secretRef = {
                name      = local.approle_auth_secret_name
                namespace = local.namespace
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

  # NOTE: This PKI ClusterSecretStore is for reading existing secrets from the
  # Vault PKI mount (e.g., CA certificates). ESO cannot issue certificates via
  # Vault PKI. For certificate issuance, use cert-manager with Vault PKI issuer.
  cluster_secret_store_pki_manifest = {
    apiVersion = "external-secrets.io/v1beta1"
    kind       = "ClusterSecretStore"
    metadata = {
      name   = local.cluster_secret_store_pki_name
      labels = local.common_labels
    }
    spec = {
      provider = {
        vault = {
          server   = local.vault_address
          path     = local.pki_mount_path
          caBundle = local.vault_ca_bundle_base64
          auth = {
            appRole = {
              path = local.approle_mount_path
              roleRef = {
                name      = local.approle_auth_secret_name
                namespace = local.namespace
                key       = "role_id"
              }
              secretRef = {
                name      = local.approle_auth_secret_name
                namespace = local.namespace
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

  webhook_pdb_manifest = {
    apiVersion = "policy/v1"
    kind       = "PodDisruptionBudget"
    metadata = {
      name      = trimspace(var.webhook_pdb_name)
      namespace = local.namespace
      labels    = local.common_labels
    }
    spec = {
      minAvailable = var.pdb_min_available
      selector = {
        matchLabels = {
          "app.kubernetes.io/name"     = "external-secrets-webhook"
          "app.kubernetes.io/instance" = local.helm_release_name
        }
      }
    }
  }

  kustomization_resources = concat(
    ["../sources/external-secrets-repo.yaml"],
    ["namespace.yaml", "helmrelease.yaml", "approle-auth-secret.yaml"],
    ["cluster-secret-store-kv.yaml"],
    local.pki_enabled ? ["cluster-secret-store-pki.yaml"] : [],
    local.webhook_pdb_enabled ? ["pdb-external-secrets-webhook.yaml"] : []
  )

  kustomization_manifest = {
    apiVersion = "kustomize.config.k8s.io/v1beta1"
    kind       = "Kustomization"
    resources  = local.kustomization_resources
  }

  rendered_manifests = merge(
    {
      "platform/sources/external-secrets-repo.yaml" = format(
        "%s\n",
        yamlencode(local.flux_helm_repository_manifest)
      )
      "platform/vault/namespace.yaml" = format(
        "%s\n",
        yamlencode(local.eso_namespace_manifest)
      )
      "platform/vault/helmrelease.yaml" = format(
        "%s\n",
        yamlencode(local.flux_helmrelease_manifest)
      )
      "platform/vault/approle-auth-secret.yaml" = format(
        "%s\n",
        yamlencode(local.approle_auth_secret_manifest)
      )
      "platform/vault/cluster-secret-store-kv.yaml" = format(
        "%s\n",
        yamlencode(local.cluster_secret_store_kv_manifest)
      )
      "platform/vault/kustomization.yaml" = format(
        "%s\n",
        yamlencode(local.kustomization_manifest)
      )
    },
    local.pki_enabled ? {
      "platform/vault/cluster-secret-store-pki.yaml" = format(
        "%s\n",
        yamlencode(local.cluster_secret_store_pki_manifest)
      )
    } : {},
    local.webhook_pdb_enabled ? {
      "platform/vault/pdb-external-secrets-webhook.yaml" = format(
        "%s\n",
        yamlencode(local.webhook_pdb_manifest)
      )
    } : {}
  )
}
