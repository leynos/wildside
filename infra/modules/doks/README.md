# DOKS Module

Provision a DigitalOcean Kubernetes (DOKS) cluster.

## Quick start

Set the DigitalOcean token so the provider can authenticate:

```sh
export DIGITALOCEAN_TOKEN="your-token"
```

Configure the provider and call the module:

```hcl
terraform {
  required_providers {
    digitalocean = {
      source  = "digitalocean/digitalocean"
      version = "~> 2.0"
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
}
```

