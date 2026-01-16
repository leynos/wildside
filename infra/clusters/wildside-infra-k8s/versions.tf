terraform {
  required_version = ">= 1.6.0, < 2.0.0"
  required_providers {
    digitalocean = {
      source  = "opentofu/digitalocean"
      version = "~> 2.66.0"
    }

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
