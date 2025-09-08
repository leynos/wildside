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
  source = "git::https://github.com/OWNER/wildside.git//infra/modules/doks?ref=v0.1.0"

  cluster_name       = "example"
  region             = "nyc1"
  kubernetes_version = "<1.33.x-do.0>" # choose a supported release

  node_pools = [{
    name       = "default"
    size       = "s-2vcpu-4gb"
    node_count = 3
    auto_scale = false
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

Select a supported Kubernetes version (1.33.x, 1.32.x, or 1.31.x). See the
DigitalOcean version table for current releases:
<https://docs.digitalocean.com/products/kubernetes/details/supported-releases/>.

Marking the `kubeconfig` output as `sensitive` hides it in the CLI but not in
state. Store state in an encrypted, access‑controlled backend and rotate the
DigitalOcean token and cluster credentials if exposure is suspected.

Enable `auto_scale` and define `min_nodes` and `max_nodes` to scale between
bounds. These settings have no effect when `auto_scale` is `false`.

Retrieve the kubeconfig and cluster endpoint after applying the configuration:

```sh
tofu output -raw kubeconfig > kubeconfig.yaml
chmod 600 kubeconfig.yaml
tofu output endpoint
```

Consult the DigitalOcean provider documentation for advanced configuration:
<https://search.opentofu.org/provider/opentofu/digitalocean/latest>

Substitute `OWNER` with the GitHub account name and `v0.1.0` with the tagged
release or commit to use.
