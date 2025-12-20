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

  webhook_helm_repository_manifest = local.webhook_release_enabled ? {
    apiVersion = "source.toolkit.fluxcd.io/v1"
    kind       = "HelmRepository"
    metadata = {
      name      = local.webhook_helm_repository_name
      namespace = local.flux_namespace
      labels    = local.common_labels
    }
    spec = merge({
      interval = local.webhook_repository_interval
      url      = local.webhook_chart_repository
      }, local.webhook_repository_type != null ? {
      type = local.webhook_repository_type
    } : {})
  } : null

  cert_manager_namespace_manifest = {
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

  webhook_helmrelease_manifest = local.webhook_release_enabled ? {
    apiVersion = "helm.toolkit.fluxcd.io/v2"
    kind       = "HelmRelease"
    metadata = {
      name      = local.webhook_release_name
      namespace = local.namespace
      labels    = local.common_labels
    }
    spec = {
      interval = local.webhook_release_interval
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
          chart = local.webhook_chart_name
          sourceRef = {
            kind      = "HelmRepository"
            name      = local.webhook_helm_repository_name
            namespace = local.flux_namespace
          }
          version = local.webhook_chart_version
        }
      }
      values = {
        replicaCount = local.webhook_release_replica_count
        groupName    = local.webhook_group_name
      }
    }
  } : null

  acme_staging_issuer_manifest = {
    apiVersion = "cert-manager.io/v1"
    kind       = "ClusterIssuer"
    metadata = {
      name   = local.acme_staging_issuer_name
      labels = local.common_labels
    }
    spec = {
      acme = {
        server = local.acme_staging_server
        email  = local.acme_email
        privateKeySecretRef = {
          name = local.acme_staging_account_key_secret_name
        }
        solvers = [local.acme_solver]
      }
    }
  }

  acme_production_issuer_manifest = {
    apiVersion = "cert-manager.io/v1"
    kind       = "ClusterIssuer"
    metadata = {
      name   = local.acme_production_issuer_name
      labels = local.common_labels
    }
    spec = {
      acme = {
        server = local.acme_production_server
        email  = local.acme_email
        privateKeySecretRef = {
          name = local.acme_production_account_key_secret_name
        }
        solvers = [local.acme_solver]
      }
    }
  }

  vault_issuer_manifest = {
    apiVersion = "cert-manager.io/v1"
    kind       = "ClusterIssuer"
    metadata = {
      name   = local.vault_issuer_name
      labels = local.common_labels
    }
    spec = {
      vault = merge({
        server = local.vault_server
        path   = local.vault_pki_path
        auth = {
          tokenSecretRef = {
            name = local.vault_token_secret_name
            key  = local.vault_token_secret_key
          }
        }
        }, local.vault_ca_bundle_base64 != null ? {
        caBundle = local.vault_ca_bundle_base64
      } : {})
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
          "app.kubernetes.io/name"     = "webhook"
          "app.kubernetes.io/instance" = local.helm_release_name
        }
      }
    }
  }

  cainjector_pdb_manifest = {
    apiVersion = "policy/v1"
    kind       = "PodDisruptionBudget"
    metadata = {
      name      = trimspace(var.cainjector_pdb_name)
      namespace = local.namespace
      labels    = local.common_labels
    }
    spec = {
      minAvailable = var.pdb_min_available
      selector = {
        matchLabels = {
          "app.kubernetes.io/name"     = "cainjector"
          "app.kubernetes.io/instance" = local.helm_release_name
        }
      }
    }
  }

  ca_bundle_secret_manifest = {
    apiVersion = "v1"
    kind       = "Secret"
    metadata = {
      name      = local.ca_bundle_secret_name
      namespace = local.namespace
      labels    = local.common_labels
    }
    type = "Opaque"
    data = {
      (local.ca_bundle_secret_key) = local.vault_ca_bundle_base64
    }
  }

  kustomization_resources = concat(
    ["namespace.yaml", "helmrelease.yaml"],
    local.webhook_release_enabled ? ["namecheap-webhook-helmrelease.yaml"] : [],
    local.webhook_pdb_enabled ? ["pdb-cert-manager-webhook.yaml"] : [],
    local.cainjector_pdb_enabled ? ["pdb-cert-manager-cainjector.yaml"] : [],
    local.acme_staging_enabled ? ["cluster-issuer-acme-staging.yaml"] : [],
    local.acme_production_enabled ? ["cluster-issuer-acme-production.yaml"] : [],
    local.vault_enabled ? ["cluster-issuer-vault.yaml"] : [],
    local.ca_bundle_secret_enabled ? ["ca-bundle.yaml"] : []
  )

  kustomization_manifest = {
    apiVersion = "kustomize.config.k8s.io/v1beta1"
    kind       = "Kustomization"
    resources  = local.kustomization_resources
  }

  rendered_manifests = merge(
    {
      "platform/sources/cert-manager-repo.yaml" = format(
        "%s\n",
        yamlencode(local.flux_helm_repository_manifest)
      )
      "platform/cert-manager/namespace.yaml" = format(
        "%s\n",
        yamlencode(local.cert_manager_namespace_manifest)
      )
      "platform/cert-manager/helmrelease.yaml" = format(
        "%s\n",
        yamlencode(local.flux_helmrelease_manifest)
      )
      "platform/cert-manager/kustomization.yaml" = format(
        "%s\n",
        yamlencode(local.kustomization_manifest)
      )
    },
    local.webhook_release_enabled ? {
      "platform/sources/namecheap-webhook-repo.yaml" = format(
        "%s\n",
        yamlencode(local.webhook_helm_repository_manifest)
      )
    } : {},
    local.webhook_release_enabled ? {
      "platform/cert-manager/namecheap-webhook-helmrelease.yaml" = format(
        "%s\n",
        yamlencode(local.webhook_helmrelease_manifest)
      )
    } : {},
    local.webhook_pdb_enabled ? {
      "platform/cert-manager/pdb-cert-manager-webhook.yaml" = format(
        "%s\n",
        yamlencode(local.webhook_pdb_manifest)
      )
    } : {},
    local.cainjector_pdb_enabled ? {
      "platform/cert-manager/pdb-cert-manager-cainjector.yaml" = format(
        "%s\n",
        yamlencode(local.cainjector_pdb_manifest)
      )
    } : {},
    local.acme_staging_enabled ? {
      "platform/cert-manager/cluster-issuer-acme-staging.yaml" = format(
        "%s\n",
        yamlencode(local.acme_staging_issuer_manifest)
      )
    } : {},
    local.acme_production_enabled ? {
      "platform/cert-manager/cluster-issuer-acme-production.yaml" = format(
        "%s\n",
        yamlencode(local.acme_production_issuer_manifest)
      )
    } : {},
    local.vault_enabled ? {
      "platform/cert-manager/cluster-issuer-vault.yaml" = format(
        "%s\n",
        yamlencode(local.vault_issuer_manifest)
      )
    } : {},
    local.ca_bundle_secret_enabled ? {
      "platform/cert-manager/ca-bundle.yaml" = format(
        "%s\n",
        yamlencode(local.ca_bundle_secret_manifest)
      )
    } : {}
  )
}
