# DOKS OpenTofu Module Design

This document outlines the initial design decisions for the DigitalOcean
Kubernetes Service (DOKS) module used to provision clusters for ephemeral
preview environments.

## Overview

The module provisions a Kubernetes cluster on DigitalOcean. It embraces
immutability and declarative configuration, allowing GitOps workflows to manage
cluster lifecycle.

## Design Decisions

- **Explicit inputs.** The module requires a cluster name, region and a
  `kubernetes_version` value plus an explicit list of node pools. This keeps
  the interface predictable and avoids hidden defaults.
- **Fail-fast validation.** Inputs for the region slug, Kubernetes version and
  node pool sizing are validated against expected patterns to catch typos and
  sizing errors before contacting the provider.
 - **High availability.** The cluster resource always enables high availability
   (`ha = true`). The module pins the DigitalOcean provider to v2.36+, where the
   `ha` argument is available, and a policy test guards against accidental
   downgrades.
- **Minimal outputs.** Only the cluster identifier, API endpoint and raw
  kubeconfig are exposed. The kubeconfig output is marked sensitive to avoid
  accidental disclosure. Consumers can derive further details from the
  kubeconfig as needed.
- **Testing strategy.** Terratest validates module syntax and exercises plan
  and apply flows. The apply step is skipped when a valid
  `DIGITALOCEAN_TOKEN` is absent, enabling local and CI execution without cloud
  credentials.

## Future Work

- Support multiple named node pools with configurable tags.
- Expose additional outputs such as the dashboard URL or VPC identifier.
- Integrate version pinning data sources once provider mocking is available.
