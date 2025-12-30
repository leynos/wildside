# Valkey module

This module deploys a [Valkey](https://valkey.io/) cluster (Redis-compatible)
using the [Hyperspike Valkey operator](https://github.com/hyperspike/valkey-operator)
to provide high-availability key-value storage. It supports TLS via cert-manager
and integrates with the `vault_eso` module for credential management via
External Secrets Operator (ESO).

## Overview

The module provisions:

- Valkey operator via Helm
- A Valkey cluster with configurable shards and replicas
- Optional TLS encryption via cert-manager
- Optional ESO integration for password management
- PodDisruptionBudget for cluster stability
- Prometheus metrics and ServiceMonitor (optional)

## Prerequisites

- A Kubernetes cluster with cluster-admin access
- OpenTofu >= 1.6.0
- `conftest` (policy tests): requires conftest built with OPA >= 0.59.0 (Rego
  v1 syntax)
- (Optional) `vault_eso` module deployed for ESO-managed credentials
- (Optional) `cert_manager` module deployed for TLS certificates

## Usage

### Render mode (GitOps)

```hcl
module "valkey" {
  source = "../valkey"

  mode = "render"

  cluster_name = "valkey"
  nodes        = 1
  replicas     = 0
}

output "rendered_manifests" {
  value     = module.valkey.rendered_manifests
  sensitive = true
}
```

### Apply mode (direct)

```hcl
provider "kubernetes" {
  config_path = var.kubeconfig_path
}

provider "helm" {
  kubernetes {
    config_path = var.kubeconfig_path
  }
}

module "valkey" {
  source = "../valkey"

  mode = "apply"

  cluster_name = "valkey"
  nodes        = 1
  replicas     = 0
}
```

### With TLS enabled

```hcl
module "valkey" {
  source = "../valkey"

  mode = "render"

  cluster_name = "valkey"
  nodes        = 1
  replicas     = 2

  # TLS via cert-manager
  tls_enabled       = true
  cert_issuer_name  = "letsencrypt-prod"
  cert_issuer_type  = "ClusterIssuer"
}
```

### With ESO credentials

```hcl
module "valkey" {
  source = "../valkey"

  mode = "render"

  cluster_name = "valkey"

  # ESO integration
  eso_enabled                   = true
  eso_cluster_secret_store_name = "vault-kv"
  password_vault_path           = "databases/valkey/password"
}
```

### High availability configuration

```hcl
module "valkey" {
  source = "../valkey"

  mode = "render"

  cluster_name = "valkey"
  nodes        = 3     # 3 shards
  replicas     = 1     # 1 replica per shard (6 pods total)

  persistence_enabled = true
  storage_size        = "5Gi"
  storage_class       = "do-block-storage"

  pdb_enabled       = true
  pdb_min_available = 2
}
```

## Inputs

### Core configuration

| Name | Description | Type | Default | Required |
|------|-------------|------|---------|:--------:|
| `mode` | Whether to render Flux manifests (`render`) or apply directly (`apply`) | `string` | `"render"` | no |
| `namespace` | Namespace for the Valkey cluster | `string` | `"valkey"` | no |
| `operator_namespace` | Namespace for the operator | `string` | `"valkey-system"` | no |
| `create_namespaces` | Whether to create namespaces | `bool` | `true` | no |
| `helm_release_name` | Name for the operator Helm release | `string` | `"valkey-operator"` | no |
| `chart_repository` | Helm repository URL | `string` | `"oci://ghcr.io/hyperspike/helm"` | no |
| `chart_name` | Helm chart name | `string` | `"valkey-operator"` | no |
| `chart_version` | Helm chart version | `string` | `"0.0.60"` | no |
| `helm_wait` | Wait for Helm release to succeed | `bool` | `true` | no |
| `helm_timeout` | Helm operation timeout (seconds) | `number` | `600` | no |
| `helm_values` | Inline YAML values for Helm | `list(string)` | `[]` | no |
| `flux_namespace` | Flux controllers namespace (render mode) | `string` | `"flux-system"` | no |
| `flux_helm_repository_name` | Flux HelmRepository name | `string` | `"valkey-operator"` | no |
| `flux_helm_repository_interval` | HelmRepository reconciliation interval | `string` | `"24h"` | no |
| `flux_helm_release_interval` | HelmRelease reconciliation interval | `string` | `"1h"` | no |

### Cluster configuration

| Name | Description | Type | Default | Required |
|------|-------------|------|---------|:--------:|
| `cluster_name` | Valkey cluster resource name | `string` | `"valkey"` | no |
| `nodes` | Number of shards (cluster nodes) | `number` | `1` | no |
| `replicas` | Replicas per shard (0 for standalone) | `number` | `0` | no |
| `cluster_domain` | Kubernetes cluster DNS domain | `string` | `"cluster.local"` | no |
| `persistence_enabled` | Enable persistent storage | `bool` | `true` | no |
| `storage_size` | PVC storage size per instance | `string` | `"1Gi"` | no |
| `storage_class` | Kubernetes StorageClass | `string` | `"do-block-storage"` | no |
| `image` | Custom Valkey image (blank for operator default) | `string` | `""` | no |
| `exporter_image` | Custom Prometheus exporter image | `string` | `""` | no |
| `resource_requests` | Pod resource requests | `object` | `{cpu="100m", memory="128Mi"}` | no |
| `resource_limits` | Pod resource limits | `object` | `{cpu="500m", memory="512Mi"}` | no |
| `prometheus_enabled` | Enable Prometheus metrics | `bool` | `false` | no |
| `service_monitor_enabled` | Create ServiceMonitor resource | `bool` | `false` | no |
| `pdb_enabled` | Create PodDisruptionBudget | `bool` | `true` | no |
| `pdb_min_available` | Minimum available pods | `number` | `1` | no |
| `pdb_name` | PDB resource name | `string` | `"pdb-valkey"` | no |
| `node_selector` | Node selector for pods | `map(string)` | `{}` | no |
| `tolerations` | Tolerations for pods | `list(object)` | `[]` | no |

### Credentials configuration

| Name | Description | Type | Default | Required |
|------|-------------|------|---------|:--------:|
| `anonymous_auth` | Allow unauthenticated access | `bool` | `false` | no |
| `password_secret_name` | Secret containing Valkey password | `string` | `"valkey-password"` | no |
| `password_secret_key` | Key in password Secret | `string` | `"password"` | no |
| `password_inline` | Inline password (if ESO not used) | `string` | `""` | no |
| `eso_enabled` | Enable ESO for password management | `bool` | `false` | no |
| `eso_cluster_secret_store_name` | ClusterSecretStore name (from `vault_eso`) | `string` | `"vault-backend"` | no |
| `eso_refresh_interval` | ExternalSecret refresh interval | `string` | `"1h"` | no |
| `password_vault_path` | Vault KV path for password | `string` | `""` | no |
| `password_vault_key` | Key in Vault secret | `string` | `"password"` | no |

### TLS configuration

| Name | Description | Type | Default | Required |
|------|-------------|------|---------|:--------:|
| `tls_enabled` | Enable TLS for connections | `bool` | `false` | no |
| `cert_issuer_name` | cert-manager issuer name | `string` | `""` | no |
| `cert_issuer_type` | Issuer type (`ClusterIssuer` or `Issuer`) | `string` | `"ClusterIssuer"` | no |
| `external_access_enabled` | Enable external access | `bool` | `false` | no |
| `external_access_type` | External access type (`loadbalancer` or `proxy`) | `string` | `"loadbalancer"` | no |

See `variables-*.tf` files for the complete list of inputs with validation
rules.

## Outputs

| Name | Description |
|------|-------------|
| `operator_namespace` | Namespace where operator is installed |
| `namespace` | Namespace where Valkey cluster runs |
| `cluster_name` | Valkey cluster resource name |
| `helm_release_name` | Operator Helm release name |
| `primary_service_name` | Kubernetes service for primary |
| `replica_service_name` | Kubernetes service for replicas |
| `primary_endpoint` | Fully qualified DNS endpoint for primary |
| `replica_endpoint` | Fully qualified DNS endpoint for replicas |
| `port` | Valkey port number |
| `credentials_secret_name` | Secret containing password (null if anonymous) |
| `credentials_secret_key` | Key in credentials Secret |
| `tls_enabled` | Whether TLS is enabled |
| `sync_policy_contract` | Contract for downstream workload consumption |
| `rendered_manifests` | GitOps manifests (render mode only) |

## Sync policy contract

The `sync_policy_contract` output provides a stable interface for downstream
workloads:

```hcl
{
  cluster = {
    name      = "valkey"
    namespace = "valkey"
  }
  endpoints = {
    primary = {
      host = "valkey.valkey.svc.cluster.local"
      port = 6379
    }
    replica = {
      host = "valkey-replica.valkey.svc.cluster.local"
      port = 6379
    }
  }
  credentials = {
    secret_name = "valkey-password"
    secret_key  = "password"
    namespace   = "valkey"
  }
  tls = {
    enabled     = true
    cert_issuer = "letsencrypt-prod"
  }
  persistence = {
    enabled       = true
    storage_class = "do-block-storage"
    size          = "1Gi"
  }
  replication = {
    nodes    = 1
    replicas = 0
  }
}
```

Applications reference this contract to obtain connection details and credential
secret names without coupling to the module's internal implementation.

## Integration with cert_manager module

When `tls_enabled = true`, the module configures TLS using a cert-manager
issuer:

```hcl
module "cert_manager" {
  source = "../cert_manager"
  # ... cert_manager configuration
}

module "valkey" {
  source = "../valkey"

  tls_enabled      = true
  cert_issuer_name = module.cert_manager.acme_production_issuer_name
  cert_issuer_type = "ClusterIssuer"
}
```

## Integration with vault_eso module

When `eso_enabled = true`, the module creates an ExternalSecret resource that
synchronises the password from Vault:

```hcl
module "vault_eso" {
  source = "../vault_eso"
  # ... vault_eso configuration
}

module "valkey" {
  source = "../valkey"

  eso_enabled                   = true
  eso_cluster_secret_store_name = module.vault_eso.cluster_secret_store_kv_name

  password_vault_path = "databases/valkey/password"
  password_vault_key  = "password"
}
```

The Vault secret should contain a key matching `password_vault_key`.

## Resources created

When `mode = "apply"`, the module creates:

1. **kubernetes_namespace.operator** – Operator namespace (optional)
2. **kubernetes_namespace.cluster** – Cluster namespace (optional)
3. **helm_release.valkey_operator** – Valkey operator
4. **kubernetes_manifest.cluster** – Valkey cluster resource
5. **kubernetes_manifest.password_secret** – Password Secret (if inline password
   provided)
6. **kubernetes_manifest.password_external_secret** – ESO ExternalSecret
   (if ESO enabled)
7. **kubernetes_manifest.pdb** – PodDisruptionBudget (if enabled)
8. **kubernetes_manifest.service_monitor** – Prometheus ServiceMonitor
   (if enabled)

## Design decisions

See [docs/valkey-module-design.md](../../../docs/valkey-module-design.md) for
detailed design decisions and rationale.
