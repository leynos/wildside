# Apply-mode example for Valkey module
#
# This example demonstrates the module in apply mode, which directly
# creates resources in a Kubernetes cluster. Requires valid kubeconfig.

variable "kubeconfig_path" {
  description = "Path to kubeconfig file for Kubernetes cluster access"
  type        = string

  validation {
    condition     = fileexists(var.kubeconfig_path)
    error_message = "kubeconfig_path must point to an existing file"
  }
}

variable "cluster_name" {
  description = "Name of the Valkey cluster resource"
  type        = string
  default     = "valkey"
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

variable "password_inline" {
  description = "Inline password value"
  type        = string
  sensitive   = true
}

provider "kubernetes" {
  config_path = var.kubeconfig_path
}

provider "helm" {
  kubernetes = {
    config_path = var.kubeconfig_path
  }
}

module "valkey" {
  source = "../.."

  mode = "apply"

  cluster_name   = var.cluster_name
  nodes          = var.nodes
  replicas       = var.replicas
  storage_size   = var.storage_size

  anonymous_auth  = false
  password_inline = var.password_inline
}

output "primary_endpoint" {
  description = "Primary endpoint for Valkey connections"
  value       = module.valkey.primary_endpoint
}

output "replica_endpoint" {
  description = "Replica endpoint for read-only connections"
  value       = module.valkey.replica_endpoint
}

output "credentials_secret_name" {
  description = "Kubernetes Secret containing the password"
  value       = module.valkey.credentials_secret_name
}

output "sync_policy_contract" {
  description = "Contract for downstream workloads"
  value       = module.valkey.sync_policy_contract
}
