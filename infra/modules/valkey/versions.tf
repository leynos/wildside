terraform {
  required_version = ">= 1.6.0, < 2.0.0"

  required_providers {
    kubernetes = {
      source  = "opentofu/kubernetes"
      version = "~> 3.0.1"
    }

    helm = {
      source  = "opentofu/helm"
      version = "~> 3.1.1"
    }
  }
}
