# Render-mode example for CloudNativePG module
#
# This example demonstrates the module in render mode, which outputs
# Flux-ready manifests for GitOps workflows. No Kubernetes cluster
# connection is required.

variable "cluster_name" {
  description = "Name of the CNPG Cluster resource"
  type        = string
  default     = "wildside-pg-main"
}

variable "instances" {
  description = "Number of PostgreSQL instances"
  type        = number
  default     = 3
}

variable "storage_size" {
  description = "PVC storage size for each instance"
  type        = string
  default     = "50Gi"
}

variable "database_name" {
  description = "Initial database name"
  type        = string
  default     = "wildside_prod"
}

variable "postgis_enabled" {
  description = "Enable PostGIS extensions"
  type        = bool
  default     = true
}

variable "backup_enabled" {
  description = "Enable S3-compatible backups"
  type        = bool
  default     = false
}

variable "backup_destination_path" {
  description = "S3 bucket path for backups"
  type        = string
  default     = ""
}

variable "backup_endpoint_url" {
  description = "S3-compatible endpoint URL"
  type        = string
  default     = ""
}

variable "backup_s3_access_key_id" {
  description = "S3 access key ID"
  type        = string
  default     = ""
  sensitive   = true
}

variable "backup_s3_secret_access_key" {
  description = "S3 secret access key"
  type        = string
  default     = ""
  sensitive   = true
}

module "cnpg" {
  source = "../.."

  mode = "render"

  cluster_name    = var.cluster_name
  instances       = var.instances
  storage_size    = var.storage_size
  database_name   = var.database_name
  postgis_enabled = var.postgis_enabled

  backup_enabled              = var.backup_enabled
  backup_destination_path     = var.backup_destination_path
  backup_endpoint_url         = var.backup_endpoint_url
  backup_s3_access_key_id     = var.backup_s3_access_key_id
  backup_s3_secret_access_key = var.backup_s3_secret_access_key
}

output "rendered_manifests" {
  description = "Rendered Flux-ready manifests"
  value       = module.cnpg.rendered_manifests
  sensitive   = true
}

output "sync_policy_contract" {
  description = "Contract for downstream workloads"
  value       = module.cnpg.sync_policy_contract
}

output "primary_endpoint" {
  description = "Primary endpoint for database connections"
  value       = module.cnpg.primary_endpoint
}
