variable "tls_enabled" {
  description = "Enable TLS for Valkey connections"
  type        = bool
  default     = false
  nullable    = false
}

variable "cert_issuer_name" {
  description = "Name of the cert-manager ClusterIssuer or Issuer for TLS certificates"
  type        = string
  default     = ""
}

variable "cert_issuer_type" {
  description = "Type of cert-manager issuer (ClusterIssuer or Issuer)"
  type        = string
  default     = "ClusterIssuer"

  validation {
    condition     = contains(["ClusterIssuer", "Issuer"], var.cert_issuer_type)
    error_message = "cert_issuer_type must be one of: ClusterIssuer, Issuer"
  }
}

variable "external_access_enabled" {
  description = "Enable external access to Valkey via LoadBalancer or Proxy"
  type        = bool
  default     = false
  nullable    = false
}

variable "external_access_type" {
  description = "Type of external access (loadbalancer or proxy)"
  type        = string
  default     = "loadbalancer"

  validation {
    condition     = contains(["loadbalancer", "proxy"], var.external_access_type)
    error_message = "external_access_type must be one of: loadbalancer, proxy"
  }
}
