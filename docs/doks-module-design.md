# DOKS OpenTofu module design

This document outlines the initial design decisions for the DigitalOcean
Kubernetes Service (DOKS) module used to provision clusters for ephemeral
preview environments.

## Overview

The module provisions a Kubernetes cluster on DigitalOcean. It embraces
immutability and declarative configuration, allowing GitOps workflows to manage
cluster lifecycle.

## Design decisions

- **Explicit inputs.** The module requires a cluster name, region and a
  `kubernetes_version` value plus an explicit list of node pools. Optional
  cluster `tags` keep the interface predictable and avoid hidden defaults.
- **Fail-fast validation.** Inputs for the region slug, Kubernetes version and
  node pool sizing are validated against expected patterns to catch typos and
  sizing errors before contacting the provider.
- **High availability.** Policy checks deny any node pool with fewer than two
  nodes, ensuring the cluster maintains a highly available control plane and
  worker set without relying on provider-specific flags.
- **Tagging.** Cluster-level tags can be supplied via the `tags` input, and
  node pool objects accept optional `tags` for cost allocation.
- **Minimal outputs.** Only the cluster identifier and API endpoint are
  exported by default. An `expose_kubeconfig` input gates the kubeconfig output,
  allowing credentials to be surfaced only when explicitly requested.
- **Testing strategy.** Terratest validates module syntax and exercises plan
  and apply flows. The apply step is skipped when a valid
  `DIGITALOCEAN_TOKEN` is absent, enabling local and CI execution without cloud
  credentials.

## Root configuration

- **Dev cluster defaults.** A root configuration in `infra/clusters/dev`
  instantiates the module with a two-node `s-2vcpu-2gb` pool in `nyc1`.
  Provisioning is gated by a `should_create_cluster` variable to avoid
  accidental applies. The Kubernetes version is sourced from the
  `DOKS_KUBERNETES_VERSION` environment variable so tooling and tests share
  one default. The kubeconfig output is disabled by default to avoid
  persisting credentials.

## Future work

- Expose additional outputs such as the dashboard URL or VPC identifier.
- Integrate version pinning data sources once provider mocking is available.
