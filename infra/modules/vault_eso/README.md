# Vault ESO Module

This module deploys [External Secrets Operator](https://external-secrets.io/)
(ESO) and configures ClusterSecretStore resources to integrate Kubernetes
workloads with an external HashiCorp Vault appliance.

## Overview

The module bridges Kubernetes workloads with the Vault appliance provisioned by
the `vault_appliance` OpenTofu module and initialized by the
`bootstrap-vault-appliance` GitHub Action. It uses AppRole authentication to
connect ESO to Vault.

## Usage

### Render Mode (GitOps)

```hcl
module "vault_eso" {
  source = "../vault_eso"

  mode = "render"

  vault_address       = "https://vault.example.com"
  vault_ca_bundle_pem = file("${path.module}/vault-ca.pem")
  approle_role_id     = var.approle_role_id
  approle_secret_id   = var.approle_secret_id
}

output "rendered_manifests" {
  value = module.vault_eso.rendered_manifests
}
```

### Apply Mode (Direct)

```hcl
provider "kubernetes" {
  config_path = var.kubeconfig_path
}

provider "helm" {
  kubernetes {
    config_path = var.kubeconfig_path
  }
}

module "vault_eso" {
  source = "../vault_eso"

  mode = "apply"

  vault_address       = "https://vault.example.com"
  vault_ca_bundle_pem = file("${path.module}/vault-ca.pem")
  approle_role_id     = var.approle_role_id
  approle_secret_id   = var.approle_secret_id
}
```

## Inputs

| Name                  | Description                                                      | Type     | Default              | Required |
| --------------------- | ---------------------------------------------------------------- | -------- | -------------------- | :------: |
| `vault_address`       | HTTPS endpoint of the external Vault appliance                   | `string` | n/a                  | yes      |
| `vault_ca_bundle_pem` | PEM-encoded Certificate Authority (CA) certificate for Vault TLS | `string` | n/a                  | yes      |
| `approle_role_id`     | AppRole role_id for ESO authentication                           | `string` | n/a                  | yes      |
| `approle_secret_id`   | AppRole secret_id for ESO authentication                         | `string` | n/a                  | yes      |
| `mode`                | `render` for GitOps manifests, `apply` for direct deployment     | `string` | `"render"`           | no       |
| `namespace`           | Namespace for ESO installation                                   | `string` | `"external-secrets"` | no       |
| `chart_version`       | ESO Helm chart version                                           | `string` | `"1.1.1"`            | no       |
| `kv_mount_path`       | Key-value (KV) v2 mount path in Vault                            | `string` | `"secret"`           | no       |

See `variables-*.tf` files for the complete list of inputs.

## Outputs

| Name                           | Description                                  |
| ------------------------------ | -------------------------------------------- |
| `namespace`                    | Namespace where ESO is installed             |
| `cluster_secret_store_kv_name` | Name of the KV ClusterSecretStore            |
| `cluster_secret_store_kv_ref`  | Reference object for the KV store            |
| `sync_policy_contract`         | Contract for downstream workload consumption |
| `rendered_manifests`           | GitOps manifests (render mode only)          |

## Sync Policy Contract

The `sync_policy_contract` output provides a stable interface for downstream
workloads:

```hcl
{
  kv_secret_store = {
    name       = "vault-kv"
    kind       = "ClusterSecretStore"
    mount_path = "secret"
  }
  vault_address         = "https://vault.example.com"
  auth_secret_name      = "vault-approle-credentials"
  auth_secret_namespace = "external-secrets"
}
```

## Creating ExternalSecrets

Application teams can create ExternalSecret resources referencing the
ClusterSecretStore:

```yaml
apiVersion: external-secrets.io/v1beta1
kind: ExternalSecret
metadata:
  name: my-app-secrets
  namespace: my-app
spec:
  refreshInterval: 1h
  secretStoreRef:
    name: vault-kv
    kind: ClusterSecretStore
  target:
    name: my-app-secrets
  data:
    - secretKey: database-password
      remoteRef:
        key: secret/data/my-app/database
        property: password
```

## Design Decisions

See [docs/vault-eso-module-design.md](../../../docs/vault-eso-module-design.md)
for detailed design decisions and rationale.
