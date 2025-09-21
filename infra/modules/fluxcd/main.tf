locals {
  namespace             = trimspace(var.namespace)
  git_repository_name   = trimspace(var.git_repository_name)
  kustomization_name    = trimspace(var.kustomization_name)
  git_repository_url    = trimspace(var.git_repository_url)
  git_repository_branch = trimspace(var.git_repository_branch)
  git_repository_path   = trimspace(var.git_repository_path)
  normalized_path = local.git_repository_path == "." ? "." : (
    startswith(local.git_repository_path, "./") ? local.git_repository_path : "./${local.git_repository_path}"
  )
  reconcile_interval    = trimspace(var.reconcile_interval)
  kustomization_timeout = trimspace(var.kustomization_timeout)
  helm_timeout          = var.helm_timeout
  helm_inline_values    = var.helm_values
  helm_value_files      = [for path in var.helm_values_files : trimspace(path)]
  helm_values           = concat(local.helm_inline_values, [for path in local.helm_value_files : file(path)])
  secret_name           = var.git_repository_secret_name == null ? null : trimspace(var.git_repository_secret_name)
  common_labels = {
    "app.kubernetes.io/managed-by" = "opentofu"
    "toolkit.fluxcd.io/provider"   = "opentofu"
  }
}

resource "kubernetes_namespace" "flux" {
  metadata {
    name = local.namespace
    labels = merge(local.common_labels, {
      "toolkit.fluxcd.io/tenant" = local.namespace
    })
  }
}

resource "helm_release" "flux" {
  name       = var.helm_release_name
  repository = var.chart_repository
  chart      = var.chart_name
  version    = var.chart_version
  namespace  = kubernetes_namespace.flux.metadata[0].name
  timeout    = local.helm_timeout
  wait       = var.helm_wait
  atomic     = true

  create_namespace = false
  cleanup_on_fail  = true
  max_history      = 3

  values = local.helm_values

  depends_on = [kubernetes_namespace.flux]
}

resource "kubernetes_manifest" "git_repository" {
  manifest = {
    apiVersion = "source.toolkit.fluxcd.io/v1"
    kind       = "GitRepository"
    metadata = {
      name      = local.git_repository_name
      namespace = kubernetes_namespace.flux.metadata[0].name
      labels    = local.common_labels
    }
    spec = merge({
      interval = local.reconcile_interval
      url      = local.git_repository_url
      }, local.git_repository_branch != "" ? {
      ref = {
        branch = local.git_repository_branch
      }
      } : {}, local.secret_name != null ? {
      secretRef = {
        name = local.secret_name
      }
    } : {})
  }

  depends_on = [helm_release.flux]
}

resource "kubernetes_manifest" "kustomization" {
  manifest = {
    apiVersion = "kustomize.toolkit.fluxcd.io/v1"
    kind       = "Kustomization"
    metadata = {
      name      = local.kustomization_name
      namespace = kubernetes_namespace.flux.metadata[0].name
      labels    = local.common_labels
    }
    spec = {
      interval = local.reconcile_interval
      path     = local.normalized_path
      prune    = var.kustomization_prune
      suspend  = var.kustomization_suspend
      timeout  = local.kustomization_timeout
      wait     = true
      sourceRef = {
        kind      = "GitRepository"
        name      = local.git_repository_name
        namespace = local.namespace
      }
    }
  }

  depends_on = [kubernetes_manifest.git_repository]
}
