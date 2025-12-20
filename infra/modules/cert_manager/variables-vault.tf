variable "vault_enabled" {
  description = "Whether to render the Vault ClusterIssuer"
  type        = bool
  default     = false
  nullable    = false
}

variable "vault_issuer_name" {
  description = "Name of the Vault ClusterIssuer"
  type        = string
  default     = "vault-issuer"

  validation {
    condition = (
      var.vault_issuer_name != null &&
      length(trimspace(var.vault_issuer_name)) > 0 &&
      can(regex("^[a-z0-9]([-a-z0-9]*[a-z0-9])?$", trimspace(var.vault_issuer_name)))
    )
    error_message = "vault_issuer_name must be a valid Kubernetes resource name"
  }
}

variable "vault_server" {
  description = "Vault server URL (must be https://)"
  type        = string
  default     = null

  validation {
    condition = (
      !var.vault_enabled ||
      (
        var.vault_server != null &&
        can(regex("^https://", trimspace(var.vault_server)))
      )
    )
    error_message = "vault_server must be an https:// URL when Vault issuer is enabled"
  }
}

variable "vault_pki_path" {
  description = "Vault PKI signing path (e.g., pki/sign/example)"
  type        = string
  default     = null

  validation {
    condition = (
      !var.vault_enabled ||
      (
        var.vault_pki_path != null &&
        length(trimspace(var.vault_pki_path)) > 0
      )
    )
    error_message = "vault_pki_path must be set when Vault issuer is enabled"
  }
}

variable "vault_token_secret_name" {
  description = "Secret containing the Vault token for cert-manager"
  type        = string
  default     = null

  validation {
    condition = (
      !var.vault_enabled ||
      (
        var.vault_token_secret_name != null &&
        length(trimspace(var.vault_token_secret_name)) > 0 &&
        can(regex(
          "^[a-z0-9]([-.a-z0-9]*[a-z0-9])?$",
          trimspace(var.vault_token_secret_name)
        ))
      )
    )
    error_message = "vault_token_secret_name must be a valid Secret name when Vault issuer is enabled"
  }
}

variable "vault_token_secret_key" {
  description = "Key in the Vault token secret containing the token"
  type        = string
  default     = "token"

  validation {
    condition = (
      !var.vault_enabled ||
      (var.vault_token_secret_key != null && length(trimspace(var.vault_token_secret_key)) > 0)
    )
    error_message = "vault_token_secret_key must not be blank when Vault issuer is enabled"
  }
}

variable "vault_ca_bundle_pem" {
  description = "PEM-encoded CA bundle for Vault TLS verification"
  type        = string
  default     = null

  validation {
    condition = (
      !var.vault_enabled ||
      (
        var.vault_ca_bundle_pem != null &&
        length(trimspace(var.vault_ca_bundle_pem)) > 0
      )
    )
    error_message = "vault_ca_bundle_pem must be provided when Vault issuer is enabled"
  }
}

variable "ca_bundle_secret_enabled" {
  description = "Whether to render/apply a Secret containing the Vault CA bundle"
  type        = bool
  default     = false
  nullable    = false

  validation {
    condition     = !var.ca_bundle_secret_enabled || var.vault_enabled
    error_message = "ca_bundle_secret_enabled requires vault_enabled"
  }
}

variable "ca_bundle_secret_name" {
  description = "Name of the Secret containing the Vault CA bundle"
  type        = string
  default     = "vault-ca-bundle"

  validation {
    condition = (
      !var.ca_bundle_secret_enabled ||
      (
        var.ca_bundle_secret_name != null &&
        length(trimspace(var.ca_bundle_secret_name)) > 0
      )
    )
    error_message = "ca_bundle_secret_name must not be blank when ca_bundle_secret_enabled is true"
  }
}

variable "ca_bundle_secret_key" {
  description = "Key in the CA bundle Secret containing the PEM data"
  type        = string
  default     = "ca.crt"

  validation {
    condition = (
      !var.ca_bundle_secret_enabled ||
      (var.ca_bundle_secret_key != null && length(trimspace(var.ca_bundle_secret_key)) > 0)
    )
    error_message = "ca_bundle_secret_key must not be blank when ca_bundle_secret_enabled is true"
  }
}
