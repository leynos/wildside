# Render-only example for the cert-manager module.
#
# This example does not require cluster access. It exercises the module's
# render mode, which emits Flux-ready YAML manifests for GitOps workflows.

variable "acme_email" {
  description = "Email address for ACME registration"
  type        = string
  default     = "platform@example.test"
}

variable "namecheap_api_secret_name" {
  description = "Secret containing Namecheap API credentials"
  type        = string
  default     = "namecheap-api-credentials"
}

variable "vault_server" {
  description = "Vault server URL"
  type        = string
  default     = "https://vault.example.test:8200"
}

variable "vault_pki_path" {
  description = "Vault PKI signing path"
  type        = string
  default     = "pki/sign/example"
}

variable "vault_token_secret_name" {
  description = "Secret containing the Vault token"
  type        = string
  default     = "vault-token"
}

variable "vault_ca_bundle_pem" {
  description = "PEM-encoded Vault CA bundle"
  type        = string
  default     = <<-PEM
    -----BEGIN CERTIFICATE-----
    MIIBpTCCAQygAwIBAgIUT5h1rn5G7p1DqVtJtB/9z7xvvi4wCgYIKoZIzj0EAwIw
    EjEQMA4GA1UEAxMHVGVzdCBDQTAeFw0yNTAxMDEwMDAwMDBaFw0zNTAxMDEwMDAw
    MDBaMBIxEDAOBgNVBAMTB1Rlc3QgQ0EwWTATBgcqhkjOPQIBBggqhkjOPQMBBwNC
    AAR1F2xEtpKX8m+u1e2Oh0ObiPrbQGdQzYz9g9FZ2qfml7yPRxYzQ/1sV9UXiYdI
    9kRvuOOAvs0wrg9pF7lzo1MwUTAdBgNVHQ4EFgQUC+eP3RzL1Xlyx6r0fAF5Oe9k
    GtIwHwYDVR0jBBgwFoAUC+eP3RzL1Xlyx6r0fAF5Oe9kGtIwDwYDVR0TAQH/BAUw
    AwEB/zAKBggqhkjOPQQDAgNJADBGAiEA0fU1cXWm5E0zB2u7k+8F8K5L2D1W7KZf
    0Sg5n2O4V5oCIQC2Dg0mEC0J3r5a0T8G8WvM3mZ3P6o7xI8V/8b7nXw1Wg==
    -----END CERTIFICATE-----
    PEM
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
  default     = "https://charts.example.test"
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

module "cert_manager" {
  source = "../.."

  mode = "render"

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

output "rendered_manifests" {
  description = "Rendered manifests keyed by GitOps path"
  value       = module.cert_manager.rendered_manifests
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
