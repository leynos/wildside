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
