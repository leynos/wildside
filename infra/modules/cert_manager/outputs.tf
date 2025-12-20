output "namespace" {
  description = "Namespace where cert-manager is installed"
  value = (
    local.is_apply_mode && var.create_namespace ? kubernetes_namespace.cert_manager[0].metadata[0].name : local.namespace
  )
}

output "helm_release_name" {
  description = "Name of the cert-manager Helm release"
  value       = local.helm_release_name
}

output "acme_staging_issuer_name" {
  description = "Name of the ACME staging ClusterIssuer"
  value       = local.acme_staging_issuer_name
}

output "acme_staging_issuer_ref" {
  description = "Reference object for the ACME staging ClusterIssuer"
  value = {
    name  = local.acme_staging_issuer_name
    kind  = "ClusterIssuer"
    group = "cert-manager.io"
  }
}

output "acme_production_issuer_name" {
  description = "Name of the ACME production ClusterIssuer"
  value       = local.acme_production_issuer_name
}

output "acme_production_issuer_ref" {
  description = "Reference object for the ACME production ClusterIssuer"
  value = {
    name  = local.acme_production_issuer_name
    kind  = "ClusterIssuer"
    group = "cert-manager.io"
  }
}

output "vault_issuer_name" {
  description = "Name of the Vault ClusterIssuer"
  value       = local.vault_issuer_name
}

output "vault_issuer_ref" {
  description = "Reference object for the Vault ClusterIssuer"
  value = {
    name  = local.vault_issuer_name
    kind  = "ClusterIssuer"
    group = "cert-manager.io"
  }
}

output "acme_staging_account_key_secret_ref" {
  description = "Secret reference for the ACME staging account private key"
  value = {
    name = local.acme_staging_account_key_secret_name
  }
}

output "acme_production_account_key_secret_ref" {
  description = "Secret reference for the ACME production account private key"
  value = {
    name = local.acme_production_account_key_secret_name
  }
}

output "namecheap_api_secret_ref" {
  description = "Secret reference holding Namecheap API credentials"
  value = {
    name     = local.namecheap_api_secret_name
    api_key  = local.namecheap_api_key_key
    api_user = local.namecheap_api_user_key
  }
}

output "vault_token_secret_ref" {
  description = "Secret reference holding the Vault token"
  value = {
    name = local.vault_token_secret_name
    key  = local.vault_token_secret_key
  }
}

output "vault_ca_bundle_pem" {
  description = "PEM-encoded Vault CA bundle"
  value       = local.vault_ca_bundle_pem
}

output "vault_ca_bundle_base64" {
  description = "Base64-encoded Vault CA bundle used in the Vault issuer"
  value       = local.vault_ca_bundle_base64
}

output "ca_bundle_secret_ref" {
  description = "Secret reference for the Vault CA bundle (if enabled)"
  value = {
    name = local.ca_bundle_secret_name
    key  = local.ca_bundle_secret_key
  }
}

output "webhook_release_name" {
  description = "Name of the Namecheap webhook Helm release"
  value       = local.webhook_release_name
}

output "rendered_manifests" {
  description = "Rendered Flux-ready manifests keyed by GitOps path (only populated when mode=render)"
  value       = local.is_render_mode ? local.rendered_manifests : {}
}
