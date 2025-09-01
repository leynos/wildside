# Ephemeral Previews Infrastructure Roadmap

This document outlines the roadmap for building the cloud-native infrastructure
required to support ephemeral preview environments for the Wildside project.
The plan is divided into distinct phases, each with a set of measurable tasks.

## Phase 1: Application Delivery and GitOps Strategy (To Do)

This phase covers the design and setup of the repositories that will manage the
application and infrastructure deployments via GitOps.

- [ ] **Finalise Application Packaging Strategy**

  - [x] **Decision**: Combine Helm for templating with Kustomize for
    environment-specific configuration.

- [ ] **Structure the `wildside-infra` Repository**

  - [ ] Create a `clusters` directory to hold the OpenTofu configurations for
    each Kubernetes cluster (e.g., clusters/dev, clusters/prod).

  - [ ] Create a `modules` directory to store reusable OpenTofu modules for
    provisioning infrastructure components (DOKS, FluxCD, etc.).

  - [ ] Create a `platform` directory containing the Kubernetes manifests (as
    Kustomizations) for core cluster services that FluxCD will manage. This
    includes:

    - `platform/sources`: For `GitRepository` and `HelmRepository` custom
      resources.

    - `platform/traefik`: `HelmRelease` for the ingress controller.

    - `platform/cert-manager`: `HelmRelease` for TLS management.

    - `platform/external-dns`: `HelmRelease` for DNS automation.

- [ ] **Structure the `wildside-apps` Repository**

  - [ ] Create a `base` directory containing the canonical HelmRelease for the
    Wildside application. This defines the default deployment configuration.

  - [ ] Create an `overlays` directory to manage environment-specific
    configurations.

  - [ ] Inside `overlays`, create directories for long-lived environments
    (`production`, `staging`). Each will contain a `kustomization.yaml` and
    patches to modify the base `HelmRelease` for that specific environment.

  - [ ] Create an `overlays/previews` directory to house the dynamically
    generated Kustomize overlays for ephemeral preview environments. This
    directory will be managed by the CI/CD pipeline.

## Phase 2: Foundational Infrastructure (To Do)

This phase focuses on provisioning the core infrastructure using the OpenTofu
modules defined in the wildside-infra repository.

### 2.1: DigitalOcean Kubernetes Cluster

- [ ] **Create a `doks` OpenTofu module**: This module will be responsible for
  provisioning the Kubernetes cluster.

- [ ] **Define input variables**: The module should accept variables for the
  cluster name, region, version, and node pool configuration.

- [ ] **Define outputs**: The module should output the cluster's kubeconfig and
  other relevant details.

- [ ] **Instantiate the module**: Create a root OpenTofu configuration that uses
  the `doks` module to provision a "dev" cluster.

- [ ] **Initialise and apply**: Run `tofu init` and `tofu apply` to create the
  cluster.

### 2.2: GitOps Control Plane

- [ ] **Create a `fluxcd` OpenTofu module**: This module will install FluxCD on
  the Kubernetes cluster.

- [ ] **Define input variables**: The module should accept variables for the Git
  repository URL and the path to the manifests.

- [ ] **Instantiate the module**: Add the fluxcd module to the root OpenTofu
  configuration.

- [ ] **Apply the changes**: Run tofu apply to install FluxCD.

### 2.3: Core Cluster Services

- [ ] **Create a `traefik` OpenTofu module**: This module will install the
  Traefik ingress controller.

- [ ] **Create an `external-dns` OpenTofu module**: This module will install
  ExternalDNS.

- [ ] **Create a `cert-manager` OpenTofu module**: This module will install
  cert-manager.

- [ ] **Create a `vault` OpenTofu module**: This module will install Vault and
  the External Secrets Operator.

- [ ] **Create a `cnpg` OpenTofu module**: This module will install
  CloudNativePG for PostgreSQL.

- [ ] **Create a `redis` OpenTofu module**: This module will install Redis.

- [ ] **Instantiate the modules**: Add the new modules to the root OpenTofu
  configuration.

- [ ] **Apply the changes**: Run `tofu apply` to install the core cluster
  services.

## Phase 3: CI/CD Workflow (In Progress)

This phase focuses on automating the lifecycle of the preview environments by
updating the wildside-apps repository, which is then actioned by FluxCD.

### 3.1: Ephemeral Environment Automation (GitHub Actions)

- [ ] **Develop a `preview-environment` reusable workflow**: This workflow will
  be triggered by pull requests in the wildside repository.

- [ ] **Build and Push Docker Images**:

  - [ ] Add steps to build `backend` and `frontend` Docker images.

  - [ ] Tag the images with the Git commit SHA.

  - [ ] Push the tagged images to a container registry (e.g., GitHub Container
    Registry).

- [ ] **Generate and Commit Kustomize Overlay**:

  - [ ] Add a step to check out the `wildside-apps` repository.

    - [ ] Create a new Kustomize overlay directory based on the pull request
      number (e.g., `overlays/previews/pr-123`).

    - [ ] Generate a `patch-helmrelease-values.yaml` file that updates the image
      tags to the new commit SHA and sets the ingress hostname.

    - [ ] Generate a `kustomization.yaml` referencing the base release and the
      patch file.

    - [ ] Update the top-level `overlays/previews/kustomization.yaml` to include
      the new preview environment.

    - [ ] Commit and push the new overlay to a branch in the `wildside-apps`
      repository.

- [ ] **Provide Feedback to the Pull Request**:

  - [ ] Add a step to post a comment on the wildside pull request with a link to
    the ephemeral preview environment.

- [ ] **Automate Environment Tear Down**:

  - [ ] Develop a separate workflow triggered on pull request closure.

  - [ ] This workflow will check out the wildside-apps repository.

  - [ ] It will remove the corresponding Kustomize overlay directory and its
    reference in the top-level kustomization.yaml.

  - [ ] It will commit and push the removal, triggering FluxCD to decommission
    the environment's resources.

### 3.2: Monitoring and Observability

- [ ] **Deploy Prometheus and Grafana**: Set up a monitoring stack to scrape
  metrics from all key cluster components.

- [ ] **Create Grafana Dashboards**: Visualise key performance indicators for
  the application and infrastructure.

- [ ] **Configure Alertmanager**: Set up alerts for critical events.

## Phase 4: Future Scalability (Not Started)

This phase includes long-term goals for enhancing the platform's capabilities.

- [ ] **Implement Multi-Cluster Management**: Extend the GitOps model to manage
  separate development, staging, and production clusters.

- [ ] **Optimise Ephemeral Database Provisioning**: Investigate and implement a
  faster method for provisioning databases for preview environments, such as
  creating a unique database or schema within a shared cluster.
