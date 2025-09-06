# DOKS Module

Provision a DigitalOcean Kubernetes (DOKS) cluster.

Requires [OpenTofu](https://opentofu.org/docs/intro/install/) 1.6 or later.

## Quick start

Set the `DIGITALOCEAN_TOKEN` environment variable to allow provider authentication:

```sh
export DIGITALOCEAN_TOKEN="<DIGITALOCEAN_TOKEN>"
```

Configure the provider, call the module, and expose outputs:

```hcl
terraform {
  required_version = ">= 1.6.0"

  required_providers {
    digitalocean = {
      source  = "opentofu/digitalocean"
      version = "~> 2.36"
    }
  }
}

provider "digitalocean" {}

module "doks" {
  source = "git::https://github.com/OWNER/wildside.git//infra/modules/doks?ref=main"

  cluster_name       = "example"
  region             = "nyc1"
  kubernetes_version = "1.28.0-do.0"

  node_pools = [{
    name       = "default"
    size       = "s-2vcpu-4gb"
    node_count = 3
    auto_scale = false
    min_nodes  = 1
    max_nodes  = 3
  }]

  expose_kubeconfig = true
}

output "endpoint" {
  value = module.doks.cluster_endpoint
}

output "kubeconfig" {
  value     = module.doks.kubeconfig
  sensitive = true
}
```

Retrieve the kubeconfig and cluster endpoint after applying the configuration:

```sh
terraform output -raw kubeconfig > kubeconfig.yaml
terraform output endpoint
```

For advanced provider configuration, consult the [DigitalOcean provider documentation](https://registry.terraform.io/providers/opentofu/digitalocean/latest/docs).

For modules published under a different account, substitute `OWNER` with the GitHub account name that hosts the repository.

