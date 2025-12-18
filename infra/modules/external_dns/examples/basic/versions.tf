# OpenTofu version constraints and provider requirements for the basic example.
#
# These must match the parent module's versions.tf to prevent provider
# namespace conflicts (e.g., hashicorp/kubernetes vs opentofu/kubernetes).

terraform {
  required_version = ">= 1.6.0, < 2.0.0"

  required_providers {
    kubernetes = {
      source  = "opentofu/kubernetes"
      version = "~> 2.25.0"
    }
    helm = {
      source  = "opentofu/helm"
      version = "~> 2.13.0"
    }
  }
}
