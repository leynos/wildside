# OpenTofu version constraints and provider requirements for the ExternalDNS
# module.

terraform {
  required_version = ">= 1.6.0, < 2.0.0"

  required_providers {
    kubernetes = {
      source  = "opentofu/kubernetes"
      version = "~> 2.38.0"
    }
    helm = {
      source  = "opentofu/helm"
      version = "~> 3.1.0"
    }
  }
}
