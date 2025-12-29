# Render-mode example for Valkey module
#
# This example demonstrates the module in render mode, which outputs
# Flux-ready manifests for GitOps workflows. No Kubernetes cluster
# connection is required.

variable "mode" {
  description = "Module mode (render or apply)"
  type        = string
  default     = "render"
}

variable "namespace" {
  description = "Namespace for the Valkey cluster"
  type        = string
  default     = "valkey"
}

variable "cluster_name" {
  description = "Name of the Valkey cluster resource"
  type        = string
  default     = "valkey"
}

variable "chart_version" {
  description = "Helm chart version for the Valkey operator"
  type        = string
  default     = "0.0.60"
}

variable "flux_helm_repository_interval" {
  description = "Interval for the Flux HelmRepository reconciliation"
  type        = string
  default     = "24h"
}

variable "cert_issuer_type" {
  description = "Type of cert-manager issuer"
  type        = string
  default     = "ClusterIssuer"
}

variable "nodes" {
  description = "Number of shards"
  type        = number
  default     = 1
}

variable "replicas" {
  description = "Number of replicas per shard"
  type        = number
  default     = 0
}

variable "storage_size" {
  description = "PVC storage size for each instance"
  type        = string
  default     = "1Gi"
}

variable "persistence_enabled" {
  description = "Enable persistent storage"
  type        = bool
  default     = true
}

variable "anonymous_auth" {
  description = "Allow anonymous access"
  type        = bool
  default     = false
}

variable "password_inline" {
  description = "Inline password value"
  type        = string
  default     = "test-password-for-render"
  sensitive   = true
}

variable "eso_enabled" {
  description = "Enable External Secrets Operator integration"
  type        = bool
  default     = false
}

variable "eso_cluster_secret_store_name" {
  description = "Name of the ClusterSecretStore for ESO"
  type        = string
  default     = "vault-backend"
}

variable "password_vault_path" {
  description = "Vault path for the Valkey password"
  type        = string
  default     = ""
}

variable "tls_enabled" {
  description = "Enable TLS"
  type        = bool
  default     = false
}

variable "cert_issuer_name" {
  description = "Name of the cert-manager issuer"
  type        = string
  default     = ""
}

variable "prometheus_enabled" {
  description = "Enable Prometheus metrics"
  type        = bool
  default     = false
}

module "valkey" {
  source = "../.."

  mode      = var.mode
  namespace = var.namespace

  cluster_name        = var.cluster_name
  nodes               = var.nodes
  replicas            = var.replicas
  storage_size        = var.storage_size
  persistence_enabled = var.persistence_enabled

  chart_version                 = var.chart_version
  flux_helm_repository_interval = var.flux_helm_repository_interval

  anonymous_auth   = var.anonymous_auth
  password_inline  = var.password_inline

  eso_enabled                   = var.eso_enabled
  eso_cluster_secret_store_name = var.eso_cluster_secret_store_name
  password_vault_path           = var.password_vault_path

  tls_enabled      = var.tls_enabled
  cert_issuer_name = var.cert_issuer_name
  cert_issuer_type = var.cert_issuer_type

  prometheus_enabled = var.prometheus_enabled
}

output "rendered_manifests" {
  description = "Rendered Flux-ready manifests"
  value       = module.valkey.rendered_manifests
  sensitive   = true
}

output "sync_policy_contract" {
  description = "Contract for downstream workloads"
  value       = module.valkey.sync_policy_contract
}

output "primary_endpoint" {
  description = "Primary endpoint for Valkey connections"
  value       = module.valkey.primary_endpoint
}

output "replica_endpoint" {
  description = "Replica endpoint for read-only connections"
  value       = module.valkey.replica_endpoint
}
