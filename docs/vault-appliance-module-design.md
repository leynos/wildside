# Vault appliance OpenTofu module design

This document captures the design decisions for the `vault_appliance` module,
which provisions a DigitalOcean-hosted Vault appliance used by the Wildside
platform for preview environment secrets management.

## Overview

The module builds a dedicated Vault footprint on DigitalOcean consisting of one
or two droplets, persistent block storage, a managed load balancer, and
hardened firewall rules. It also generates the TLS assets and recovery key
material required to bootstrap Vault in a deterministic, GitOps-friendly way.

## Design decisions

- **Deterministic naming and tagging.** All resources derive their names from a
  validated `name` prefix. The module normalizes characters, truncates to
  provider limits, and adds consistent tags (`vault` plus the base name) so
  supporting automation can discover the appliance reliably.
- **HA-aware topology.** A single toggle (`ha_enabled`) controls whether to
  create one droplet or an HA pair. Storage volumes, firewall rules, and load
  balancer attachments are rendered from the same local metadata to guarantee
  that the infrastructure scales coherently when the toggle flips.
- **Persistent storage by default.** Each droplet receives its own
  `digitalocean_volume`, mounted through `digitalocean_volume_attachment`. The
  volumes default to 50 GiB with an overridable filesystem label, ensuring
  future bootstrap automation can format and mount the disks predictably.
- **Managed TLS chain.** A private CA and server certificate are generated with
  the TLS provider. The server keypair is uploaded to DigitalOcean as a custom
  certificate and exposed via outputs so the bootstrap helper can install the
  same bundle on the droplets. The CA certificate is returned for callers to
  trust the load balancer endpoint.
- **Recovery material baked in.** Recovery keys are modelled as
  `random_password` resources. Callers can tune share count, threshold, and key
  length, enabling the bootstrap workflow to unseal Vault without manual
  intervention. The keys are emitted as sensitive outputs to encourage storage
  in a secure backend.
- **Secure perimeter.** The module provisions a firewall that only accepts SSH
  from explicitly listed CIDRs and API traffic from the managed load balancer.
  Conftest policies enforce HTTPS termination, HTTP→HTTPS redirects, and forbid
  exposing SSH to the public internet by default. Additional inputs allow the
  appliance to live inside a custom VPC and project.
- **Detailed drift checks.** Behavioural tests execute
  `tofu plan -detailed-exitcode` to verify that creating the appliance produces
  exit code 2, proving the safety rails that guard destructive operations. The
  Makefile mirrors this check for local and CI runs.
- **Terratest coverage.** The test suite validates happy and unhappy paths:
  provider authentication failures, recovery threshold validation, policy
  enforcement (including negative cases), HA topology rendering, detailed exit
  codes, and an opt-in live apply guarded by `VAULT_APPLIANCE_ACCEPT_APPLY`.
- **Project integration.** An optional `project_id` input assigns the droplet,
  volumes, and load balancer to a DigitalOcean project via
  `digitalocean_project_resources`, keeping billing and governance aligned with
  wider platform conventions.

## Future work

- Extend the module to template droplet user data for automatic volume mounts
  and Vault configuration scaffolding.
- Publish metrics (load balancer health, droplet monitoring) to a central
  observability stack once platform monitoring is formalised.
- Support BYO certificate chains for teams that integrate Vault with an
  external PKI, while retaining the default self-signed path for rapid
  bootstrap.
