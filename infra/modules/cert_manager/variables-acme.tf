variable "acme_email" {
  description = "Email address registered with the ACME certificate authority"
  type        = string
  nullable    = false

  validation {
    condition     = var.acme_email != null && can(regex("^[^@]+@[^@]+\\.[^@]+$", trimspace(var.acme_email)))
    error_message = "acme_email must be a valid email address"
  }
}

variable "acme_staging_enabled" {
  description = "Whether to render the Let's Encrypt staging ClusterIssuer"
  type        = bool
  default     = true
  nullable    = false
}

variable "acme_production_enabled" {
  description = "Whether to render the Let's Encrypt production ClusterIssuer"
  type        = bool
  default     = true
  nullable    = false
}

variable "acme_staging_server" {
  description = "ACME server URL for Let's Encrypt staging"
  type        = string
  default     = "https://acme-staging-v02.api.letsencrypt.org/directory"

  validation {
    condition = (
      var.acme_staging_server != null &&
      can(regex("^https://", trimspace(var.acme_staging_server)))
    )
    error_message = "acme_staging_server must be an https:// URL"
  }
}

variable "acme_production_server" {
  description = "ACME server URL for Let's Encrypt production"
  type        = string
  default     = "https://acme-v02.api.letsencrypt.org/directory"

  validation {
    condition = (
      var.acme_production_server != null &&
      can(regex("^https://", trimspace(var.acme_production_server)))
    )
    error_message = "acme_production_server must be an https:// URL"
  }
}

variable "acme_staging_issuer_name" {
  description = "Name of the ACME staging ClusterIssuer"
  type        = string
  default     = "letsencrypt-staging"

  validation {
    condition = (
      var.acme_staging_issuer_name != null &&
      length(trimspace(var.acme_staging_issuer_name)) > 0 &&
      can(regex("^[a-z0-9]([-a-z0-9]*[a-z0-9])?$", trimspace(var.acme_staging_issuer_name)))
    )
    error_message = "acme_staging_issuer_name must be a valid Kubernetes resource name"
  }
}

variable "acme_production_issuer_name" {
  description = "Name of the ACME production ClusterIssuer"
  type        = string
  default     = "letsencrypt-production"

  validation {
    condition = (
      var.acme_production_issuer_name != null &&
      length(trimspace(var.acme_production_issuer_name)) > 0 &&
      can(regex("^[a-z0-9]([-a-z0-9]*[a-z0-9])?$", trimspace(var.acme_production_issuer_name)))
    )
    error_message = "acme_production_issuer_name must be a valid Kubernetes resource name"
  }
}

variable "acme_staging_account_key_secret_name" {
  description = "Secret name storing the ACME staging account private key"
  type        = string
  default     = "letsencrypt-staging-account-key"

  validation {
    condition = (
      var.acme_staging_account_key_secret_name != null &&
      length(trimspace(var.acme_staging_account_key_secret_name)) > 0 &&
      can(regex(
        "^[a-z0-9]([-.a-z0-9]*[a-z0-9])?$",
        trimspace(var.acme_staging_account_key_secret_name)
      ))
    )
    error_message = "acme_staging_account_key_secret_name must be a valid Secret name"
  }
}

variable "acme_production_account_key_secret_name" {
  description = "Secret name storing the ACME production account private key"
  type        = string
  default     = "letsencrypt-production-account-key"

  validation {
    condition = (
      var.acme_production_account_key_secret_name != null &&
      length(trimspace(var.acme_production_account_key_secret_name)) > 0 &&
      can(regex(
        "^[a-z0-9]([-.a-z0-9]*[a-z0-9])?$",
        trimspace(var.acme_production_account_key_secret_name)
      ))
    )
    error_message = "acme_production_account_key_secret_name must be a valid Secret name"
  }
}

variable "webhook_group_name" {
  description = "API group name used by the Namecheap DNS-01 webhook solver"
  type        = string
  default     = "acme.example.com"

  validation {
    condition     = var.webhook_group_name != null && length(trimspace(var.webhook_group_name)) > 0
    error_message = "webhook_group_name must not be blank"
  }
}

variable "webhook_solver_name" {
  description = "Solver name registered by the Namecheap DNS-01 webhook"
  type        = string
  default     = "namecheap"

  validation {
    condition     = var.webhook_solver_name != null && length(trimspace(var.webhook_solver_name)) > 0
    error_message = "webhook_solver_name must not be blank"
  }
}

variable "namecheap_api_secret_name" {
  description = "Secret containing Namecheap API credentials"
  type        = string
  default     = null

  validation {
    condition = (
      (!var.acme_staging_enabled && !var.acme_production_enabled) ||
      (
        var.namecheap_api_secret_name != null &&
        length(trimspace(var.namecheap_api_secret_name)) > 0 &&
        can(regex(
          "^[a-z0-9]([-.a-z0-9]*[a-z0-9])?$",
          trimspace(var.namecheap_api_secret_name)
        ))
      )
    )
    error_message = "namecheap_api_secret_name must be a valid Secret name when ACME issuers are enabled"
  }
}

variable "namecheap_api_key_key" {
  description = "Key in the Namecheap secret containing the API key"
  type        = string
  default     = "api-key"

  validation {
    condition     = var.namecheap_api_key_key != null && length(trimspace(var.namecheap_api_key_key)) > 0
    error_message = "namecheap_api_key_key must not be blank"
  }
}

variable "namecheap_api_user_key" {
  description = "Key in the Namecheap secret containing the API user"
  type        = string
  default     = "api-user"

  validation {
    condition     = var.namecheap_api_user_key != null && length(trimspace(var.namecheap_api_user_key)) > 0
    error_message = "namecheap_api_user_key must not be blank"
  }
}
