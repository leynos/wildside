# FluxCD Module

Install the Flux GitOps toolkit on an existing Kubernetes cluster using
OpenTofu and Helm.

Requires [OpenTofu](https://opentofu.org/docs/intro/install/) 1.6 or later and
access to a Kubernetes cluster. The module configures Flux to track a Git
repository and reconcile a specified sub-path.

## Quick start

Provide a kubeconfig file that grants cluster-admin privileges for the target
cluster:

```sh
export KUBECONFIG="/path/to/kubeconfig"
```

Configure the providers, call the module, and expose outputs:

```hcl
terraform {
  required_version = ">= 1.6.0"

  required_providers {
    kubernetes = {
      source  = "opentofu/kubernetes"
      version = "~> 2.25"
    }
    helm = {
      source  = "opentofu/helm"
      version = "~> 2.13"
    }
  }
}

provider "kubernetes" {}

provider "helm" {
  kubernetes {}
}

module "fluxcd" {
  # Prefer a released tag or commit SHA for reproducibility
  source = "git::https://github.com/OWNER/wildside.git//infra/modules/fluxcd?ref=<TAG_OR_SHA>"

  git_repository_url    = "https://github.com/OWNER/wildside-infra.git"
  git_repository_branch = "main"
  git_repository_path   = "clusters/dev"
  reconcile_interval    = "1m"
}

output "flux_namespace" {
  value = module.fluxcd.namespace
}
```

The module installs the [`flux2` Helm chart](https://github.com/fluxcd-community/helm-charts)
into the `flux-system` namespace and creates Flux `GitRepository` and
`Kustomization` resources referencing the supplied Git repository and path.

> Caution: Flux requires that the configured Git repository is reachable from
> the cluster. Provide SSH credentials via the `git_repository_secret_name`
> input and an accompanying Kubernetes secret when using private repositories.

Retrieve the objects after applying the configuration:

```sh
tofu output namespace
tofu output git_repository_name
tofu output kustomization_name
```

Refer to the [Flux documentation](https://fluxcd.io/docs/) for options such as
multi-tenancy lockdown and image automation. Override the Helm chart values via
`helm_release` arguments in a wrapper module if you need additional
customisation.
