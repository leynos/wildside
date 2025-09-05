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
- **High availability.** The cluster resource always enables high availability
  (`ha = true`). A policy test enforces this to guard against accidental
  downgrades.
- **Minimal outputs.** Only the cluster identifier, API endpoint and raw
  kubeconfig are exposed. Consumers can derive further details from the
  kubeconfig as needed.
- **Testing strategy.** Terratest validates module syntax and exercises plan
  and apply flows. The apply step is skipped when a valid
  `DIGITALOCEAN_TOKEN` is absent, enabling local and CI execution without cloud
  credentials.

## Future Work

- Support multiple named node pools with configurable tags.
- Expose additional outputs such as the dashboard URL or VPC identifier.
- Integrate version pinning data sources once provider mocking is available.
