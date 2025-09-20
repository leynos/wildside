# FluxCD OpenTofu module design

This document records the design decisions behind the FluxCD module that
installs the GitOps control plane for Wildside's ephemeral preview platform.

## Overview

The module installs Flux via the community `flux2` Helm chart and provisions
GitOps primitives (a `GitRepository` and `Kustomization`) so the cluster
immediately begins reconciling the desired state stored in Git. The module is
meant to run against an existing Kubernetes cluster provisioned by the DOKS
module.

## Design decisions

- **Helm-driven installation.** The module uses the `flux2` chart from the
  `fluxcd-community` repository. Helm handles controller lifecycle management,
  CRD installation, and upgrades while leaving Git integration to native Flux
  custom resources created via the Kubernetes provider.
- **Declarative GitOps bootstrap.** The module defines a Flux `GitRepository`
  and `Kustomization` resource. This mirrors the `flux bootstrap` workflow
  without shelling out to the Flux CLI. The resources are parameterised so the
  Git URL, branch, and repository path are supplied by callers.
- **Safe defaults.** Reconciliation runs every minute, pruning is enabled, and
  the module prevents suspended Kustomizations or absolute repository paths via
  input validation and OPA policies. Optional inputs allow callers to supply a
  Kubernetes secret for private repositories.
- **Dual authentication paths.** The parent configuration may either provide a
  kubeconfig path or allow the module to derive credentials from the DOKS API.
  When credentials are fetched remotely, the module decodes the DigitalOcean
  cluster's CA certificate and token and wires them into aliased Helm and
  Kubernetes providers.
- **Policy enforcement.** A Conftest policy ensures Git URLs use an accepted
  scheme, reconciliation intervals stay within seconds or minutes, pruning is
  enabled, and the Kustomization references a GitRepository source. This aligns
  with the GitOps conventions defined for Wildside.
- **Testing strategy.** Terratest covers validation failures, provider errors
  (missing kubeconfig), detailed exit codes, Conftest enforcement, and a gated
  apply flow that runs only when `FLUXCD_ACCEPT_APPLY` and `KUBECONFIG` are set.
  Tests reuse the shared `infra/testutil` helpers for consistency across
  modules.

## Integration with the dev cluster configuration

- **Conditional install.** `infra/clusters/dev` exposes a `should_install_flux`
  flag. When enabled, the configuration instantiates the Flux module and
  configures Helm/Kubernetes providers using either an operator-supplied
  kubeconfig file or the cluster credentials fetched from DigitalOcean.
- **Validations.** The root module validates that enabling Flux requires a
  DOKS cluster and a Git repository URL. Tests assert these guard rails.
- **Outputs.** The dev configuration now exports the Flux namespace, Git
  repository name, and Kustomization name to simplify downstream automation.

## Future work

- Surface additional Flux configuration (notification controllers, image
  automation) as optional inputs once the GitOps tree is fully modelled.
- Expand policy coverage to assert tighter reconciliation SLAs or enforce
  multi-tenancy lockdown once platform requirements are finalised.
