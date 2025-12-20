# Apply-mode example for the cert-manager module.
#
# This example deploys cert-manager directly to a Kubernetes cluster. It
# requires a kubeconfig with cluster-admin access.

variable "kubeconfig_path" {
  description = "Path to a kubeconfig file with cluster-admin access"
  type        = string
  default     = null
  validation {
    condition = (
      var.kubeconfig_path == null ||
      (
        trimspace(var.kubeconfig_path) != "" &&
        fileexists(trimspace(var.kubeconfig_path))
      )
    )
    error_message = "Set kubeconfig_path to a readable kubeconfig file before running the example"
  }
}

variable "acme_email" {
  description = "Email address for ACME registration"
  type        = string

  validation {
    condition     = var.acme_email != null && can(regex("^[^@]+@[^@]+\\.[^@]+$", trimspace(var.acme_email)))
    error_message = "acme_email must be a valid email address"
  }
}

variable "namecheap_api_secret_name" {
  description = "Secret containing Namecheap API credentials"
  type        = string

  validation {
    condition     = var.namecheap_api_secret_name != null && length(trimspace(var.namecheap_api_secret_name)) > 0
    error_message = "namecheap_api_secret_name must be a non-empty string"
  }
}

variable "vault_server" {
  description = "Vault server URL"
  type        = string

  validation {
    condition     = var.vault_server != null && can(regex("^https://", trimspace(var.vault_server)))
    error_message = "vault_server must be an https:// URL"
  }
}

variable "vault_pki_path" {
  description = "Vault PKI signing path"
  type        = string

  validation {
    condition     = var.vault_pki_path != null && length(trimspace(var.vault_pki_path)) > 0
    error_message = "vault_pki_path must be a non-empty string"
  }
}

variable "vault_token_secret_name" {
  description = "Secret containing the Vault token"
  type        = string

  validation {
    condition     = var.vault_token_secret_name != null && length(trimspace(var.vault_token_secret_name)) > 0
    error_message = "vault_token_secret_name must be a non-empty string"
  }
}

variable "vault_ca_bundle_pem" {
  description = "PEM-encoded Vault CA bundle"
  type        = string

  validation {
    condition     = var.vault_ca_bundle_pem != null && length(trimspace(var.vault_ca_bundle_pem)) > 0
    error_message = "vault_ca_bundle_pem must be a non-empty string"
  }
}

# Optional variables with defaults matching the module, except where noted to
# keep this example Vault-enabled by default.
#
# These defaults intentionally duplicate the module's defaults to enable test
# flexibility. Tests can override these variables without redefining the
# module's interface in this root module.

variable "namespace" {
  description = "Namespace where cert-manager will be installed"
  type        = string
  default     = "cert-manager"
}

variable "vault_enabled" {
  description = "Whether to enable the Vault ClusterIssuer"
  type        = bool
  default     = true
}

variable "webhook_release_enabled" {
  description = "Whether to deploy the Namecheap webhook Helm release"
  type        = bool
  default     = false
}

variable "webhook_chart_repository" {
  description = "Helm repository hosting the Namecheap webhook chart"
  type        = string
  default     = null
}

variable "webhook_chart_name" {
  description = "Name of the Namecheap webhook chart"
  type        = string
  default     = "cert-manager-webhook-namecheap"
}

variable "webhook_chart_version" {
  description = "Chart version for the Namecheap webhook"
  type        = string
  default     = "0.2.0"
}

variable "webhook_repository_type" {
  description = "Optional repository type for the Namecheap webhook HelmRepository"
  type        = string
  default     = null
}

variable "webhook_group_name" {
  description = "API group name used by the Namecheap webhook solver"
  type        = string
  default     = "acme.example.com"
}

variable "webhook_solver_name" {
  description = "Solver name registered by the Namecheap webhook"
  type        = string
  default     = "namecheap"
}

variable "namecheap_api_key_key" {
  description = "Key in the Namecheap secret containing the API key"
  type        = string
  default     = "api-key"
}

variable "namecheap_api_user_key" {
  description = "Key in the Namecheap secret containing the API user"
  type        = string
  default     = "api-user"
}

variable "vault_token_secret_key" {
  description = "Key in the Vault token secret containing the token"
  type        = string
  default     = "token"
}

variable "ca_bundle_secret_enabled" {
  description = "Whether to render a Secret containing the Vault CA bundle"
  type        = bool
  default     = false
}

variable "ca_bundle_secret_name" {
  description = "Name of the Secret containing the Vault CA bundle"
  type        = string
  default     = "vault-ca-bundle"
}

variable "ca_bundle_secret_key" {
  description = "Key in the CA bundle Secret containing the PEM data"
  type        = string
  default     = "ca.crt"
}

provider "kubernetes" {
  config_path = var.kubeconfig_path != null ? trimspace(var.kubeconfig_path) : null
}

provider "helm" {
  kubernetes {
    config_path = var.kubeconfig_path != null ? trimspace(var.kubeconfig_path) : null
  }
}

module "cert_manager" {
  source = "../.."

  mode = "apply"

  namespace                 = var.namespace
  acme_email                = var.acme_email
  namecheap_api_secret_name = var.namecheap_api_secret_name
  webhook_group_name        = var.webhook_group_name
  webhook_solver_name       = var.webhook_solver_name
  namecheap_api_key_key     = var.namecheap_api_key_key
  namecheap_api_user_key    = var.namecheap_api_user_key

  webhook_release_enabled  = var.webhook_release_enabled
  webhook_chart_repository = var.webhook_chart_repository
  webhook_chart_name       = var.webhook_chart_name
  webhook_chart_version    = var.webhook_chart_version
  webhook_repository_type  = var.webhook_repository_type

  vault_enabled            = var.vault_enabled
  vault_server             = var.vault_server
  vault_pki_path           = var.vault_pki_path
  vault_token_secret_name  = var.vault_token_secret_name
  vault_token_secret_key   = var.vault_token_secret_key
  vault_ca_bundle_pem      = var.vault_ca_bundle_pem
  ca_bundle_secret_enabled = var.ca_bundle_secret_enabled
  ca_bundle_secret_name    = var.ca_bundle_secret_name
  ca_bundle_secret_key     = var.ca_bundle_secret_key
}

output "namespace" {
  description = "Namespace where cert-manager is installed"
  value       = module.cert_manager.namespace
}

output "acme_staging_issuer_name" {
  description = "Name of the ACME staging ClusterIssuer"
  value       = module.cert_manager.acme_staging_issuer_name
}

output "acme_production_issuer_name" {
  description = "Name of the ACME production ClusterIssuer"
  value       = module.cert_manager.acme_production_issuer_name
}

output "vault_issuer_name" {
  description = "Name of the Vault ClusterIssuer"
  value       = module.cert_manager.vault_issuer_name
}
