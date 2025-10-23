# Ephemeral previews infrastructure roadmap

This document outlines the roadmap for building the cloud-native infrastructure
required to support ephemeral preview environments for the Wildside project.
The plan is divided into distinct phases, each with a set of measurable tasks.

## Phase 1: Application delivery and GitOps strategy (To do)

This phase covers the design and setup of the repositories that will manage the
application and infrastructure deployments via GitOps.

- [ ] **Finalize application packaging strategy**

  - [x] **Decision**: Combine Helm for templating with Kustomize for
    environment-specific configuration.

## Phase 2: Foundational infrastructure (To do)

This phase focuses on provisioning the core infrastructure using the OpenTofu
modules defined in the wildside-infra repository.

### 2.1: DigitalOcean Kubernetes cluster

- [x] **Create a `doks` OpenTofu module**: This module will be responsible for
  provisioning the Kubernetes cluster.

- [x] **Define input variables**: The module should accept variables for the
  cluster name, region, version, and node pool configuration.

- [x] **Define outputs**: The module outputs the cluster ID and API endpoint.
  The kubeconfig can be optionally exposed for local use.

- [x] **Instantiate the module**: [`infra/clusters/dev/main.tf`](../infra/clusters/dev/main.tf)
  uses the `doks` module to provision a "dev" cluster. Provisioning is gated
  by setting `TF_VAR_should_create_cluster=true`.

- [ ] **Initialize and apply**: Run `tofu init` and `tofu apply` to create the
  cluster.

### 2.2: GitOps control plane

- [x] **Create a `fluxcd` OpenTofu module**: This module will install FluxCD on
  the Kubernetes cluster.

- [x] **Define input variables**: The module should accept variables for the Git
  repository URL and the path to the manifests.

- [x] **Instantiate the module**: Add the fluxcd module to the root OpenTofu
  configuration.

- [ ] **Apply the changes**: Run `tofu apply` to install FluxCD.

### 2.3: Core cluster services

These tasks deliver the shared fixtures that `wildside-infra-k8s` converges on
each time it runs. The action consumes OpenTofu modules from the `infra`
repository and commits Flux-ready manifests into `wildside-infra`.

- [ ] **Publish reusable OpenTofu modules**: Add modules for Traefik,
  ExternalDNS, cert-manager, Vault + External Secrets Operator, CloudNativePG,
  and Redis under `infra/modules`. Each module should expose inputs/outputs the
  action can pass between components (e.g., DNS zones, certificate issuers,
  database connection details).

- [ ] **Lay out the `wildside-infra` GitOps tree**: Ensure the repository hosts
  `clusters/<cluster>/`, `modules/`, and a `platform` directory with
  subdirectories for `sources`, `traefik`, `cert-manager`, `external-dns`,
  `vault`, and shared data services (CloudNativePG, Redis). Each subdirectory
  should contain the HelmReleases, Kustomizations, and supporting manifests the
  action can render idempotently.

- [ ] **Extend `wildside-infra-k8s` for fixtures**: Update the action so it
  applies the new modules, writes resulting manifests into the `platform/*`
  directories, commits any drift, and sources required secrets from HashiCorp
  Vault. Rerunning the action must reconcile the repository state without
  manual `tofu apply` steps.

### 2.4: Vault appliance bootstrap

- [x] **Author an OpenTofu `vault_appliance` module**: Provision a dedicated
  DigitalOcean Droplet (or HA pair) with firewall rules, block storage, and
  load balancer attachment for Vault. Expose outputs for the public endpoint,
  CA certificate, and recovery keys.

- [x] **Ship a Python bootstrap helper**: Create
  `scripts/bootstrap_vault_appliance.py` using the
  [scripting standards](scripting-standards.md) to initialise Vault (init if
  sealed, unseal with stored shares, enable the KV v2 mount, and provision the
  AppRole used by the DOKS workflow). Include pytest coverage with
  `cmd-mox`-backed mocks for `vault`, `doctl`, and `ssh` interactions.

- [ ] **Publish a `bootstrap-vault-appliance` GitHub Action**: Wrap the Python
  helper in a composite action that the manual DOKS workflow and the
  `wildside-infra-k8s` pipeline can reuse. The action should accept environment
  identifiers, Vault seal key secrets, and DigitalOcean credentials, and must
  be idempotent so re-runs verify rather than recreate the appliance.

## Phase 3: CI/CD workflow (In progress)

This phase focuses on automating the lifecycle of the ephemeral preview
environments by updating the wildside-apps repository, which is then actioned
by FluxCD.

### 3.1: Reusable idempotent actions

These actions converge repository state idempotently, committing changes to
their respective GitOps repositories while sourcing secrets from a shared
HashiCorp Vault instance.

- [ ] **Develop the `wildside-infra-k8s` action**:

  - [ ] Assemble Kubernetes clusters and shared fixtures from the OpenTofu
    modules in the Wildside repository, persisting the resulting state in the
    `wildside-infra` GitOps repository for FluxCD to reconcile.

  - [ ] Ensure the repository contains the expected GitOps layout:

    - `clusters` for per-cluster OpenTofu configurations.

    - `modules` for reusable OpenTofu modules (DOKS, FluxCD, etc.).

    - `platform` Kustomizations for core cluster services, including
      `sources`, `traefik`, `cert-manager`, `external-dns`, and `vault`
      subdirectories.

  - [ ] Render Helm-based fixtures into the `platform/*` tree so FluxCD
    applies them and retrieve any required secrets from HashiCorp Vault.

- [ ] **Develop the `wildside-app` action**:

  - [ ] Deploy an application instance onto an existing cluster by generating
    overlays in the `wildside-apps` repository and committing the desired state
    for FluxCD.

  - [ ] Maintain the repository structure:

    - `base` containing the canonical `HelmRelease` for the application.

    - `overlays` with long-lived environments (`production`, `staging`) and an
      `overlays/ephemeral/` directory for dynamically generated overlays.

  - [ ] Fetch application secrets from HashiCorp Vault and reference them in
    the rendered manifests.

### 3.2: Ephemeral environment automation (GitHub Actions)

- [ ] **Develop an `ephemeral-environment` reusable workflow**: This workflow
  will be triggered by pull requests in the wildside repository.

- [ ] **Build and push Docker images**:

  - [ ] Add steps to build `backend` and `frontend` Docker images.

  - [ ] Tag the images with the Git commit SHA.

  - [ ] Push the tagged images to a container registry (e.g., GitHub Container
    Registry).

- [ ] **Generate and commit Kustomize overlay**:

  - [ ] Add a step to check out the `wildside-apps` repository.

    - [ ] Create a new Kustomize overlay directory based on the pull request
      number (e.g., `overlays/ephemeral/pr-123`).

    - [ ] Generate a `patch-helmrelease-values.yaml` file that updates the image
      tags to the new commit SHA and sets the ingress hostname.

    - [ ] Generate a `kustomization.yaml` referencing the base release and the
      patch file.

    - [ ] Update the top-level `overlays/ephemeral/kustomization.yaml` to include
      the new ephemeral environment.

    - [ ] Commit and push the new overlay to a branch in the `wildside-apps`
      repository.

- [ ] **Provide feedback to the pull request**:

  - [ ] Add a step to post a comment on the wildside pull request with a link to
    the ephemeral preview environment.

- [ ] **Automate environment teardown**:

  - [ ] Develop a separate workflow triggered on pull request closure.

  - [ ] This workflow will check out the wildside-apps repository.

  - [ ] It will remove the corresponding Kustomize overlay directory and its
    reference in the top-level kustomization.yaml.

  - [ ] It will commit and push the removal, triggering FluxCD to decommission
    the environment's resources.

### 3.3: Monitoring and observability

- [ ] **Deploy Prometheus and Grafana**: Set up a monitoring stack to scrape
  metrics from all key cluster components.

- [ ] **Create Grafana dashboards**: Visualize key performance indicators for
  the application and infrastructure.

- [ ] **Configure Alertmanager**: Set up alerts for critical events.

## Phase 4: Future scalability (Not started)

This phase includes long-term goals for enhancing the platform's capabilities.

- [ ] **Implement multi-cluster management**: Extend the GitOps model to manage
  separate development, staging, and production clusters.

- [ ] **Optimize ephemeral database provisioning**: Investigate and implement a
  faster method for provisioning databases for ephemeral environments, such as
  creating a unique database or schema within a shared cluster.
