# Full example for the platform_render module.
#
# This example enables all platform modules and demonstrates the complete
# configuration surface. No Kubernetes cluster connection is required;
# the module operates entirely in render mode.

variable "cluster_name" {
  description = "Name of the Kubernetes cluster"
  type        = string
  default     = "preview-example"
}

variable "domain" {
  description = "Root DNS domain for the cluster"
  type        = string
  default     = "example.test"
}

variable "acme_email" {
  description = "Email address for ACME certificate registration"
  type        = string
  default     = "platform@example.test"
}

variable "cloudflare_api_token_secret_name" {
  description = "Name of the Kubernetes secret containing the Cloudflare API token"
  type        = string
  default     = "cloudflare-api-token"
}

variable "vault_address" {
  description = "HTTPS endpoint of the external Vault appliance"
  type        = string
  default     = "https://vault.example.test:8200"
}

variable "vault_ca_bundle_pem" {
  description = "PEM-encoded CA certificate for Vault TLS verification"
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

variable "vault_approle_role_id" {
  description = "AppRole role_id for Vault authentication"
  type        = string
  default     = "test-role-id-12345678-1234-1234-1234-123456789012"
}

variable "vault_approle_secret_id" {
  description = "AppRole secret_id for Vault authentication"
  type        = string
  default     = "test-secret-id-12345678-1234-1234-1234-123456789012"
  sensitive   = true
}

module "platform" {
  source = "../.."

  cluster_name = var.cluster_name
  domain       = var.domain
  acme_email   = var.acme_email

  # Cloudflare configuration
  cloudflare_api_token_secret_name = var.cloudflare_api_token_secret_name

  # Vault configuration
  vault_address           = var.vault_address
  vault_ca_bundle_pem     = var.vault_ca_bundle_pem
  vault_approle_role_id   = var.vault_approle_role_id
  vault_approle_secret_id = var.vault_approle_secret_id

  # Enable platform modules (valkey stays disabled by default in this example).
  traefik_enabled      = true
  cert_manager_enabled = true
  external_dns_enabled = true
  vault_eso_enabled    = true
  cnpg_enabled         = true
  valkey_enabled       = false
}

output "rendered_manifests" {
  description = "Merged Flux-ready manifests keyed by GitOps path"
  value       = module.platform.rendered_manifests
  sensitive   = true
}

output "manifest_count" {
  description = "Total number of rendered manifests"
  value       = module.platform.manifest_count
}

output "manifest_counts_by_module" {
  description = "Manifest counts grouped by module"
  value       = module.platform.manifest_counts_by_module
}

output "enabled_modules" {
  description = "List of enabled platform modules"
  value       = module.platform.enabled_modules
}

output "traefik_ingress_class_name" {
  description = "Name of the Traefik IngressClass"
  value       = module.platform.traefik_ingress_class_name
}

output "cnpg_primary_endpoint" {
  description = "Primary endpoint for CNPG database connections"
  value       = module.platform.cnpg_primary_endpoint
}
