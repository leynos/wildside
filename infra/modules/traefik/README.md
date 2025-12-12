# Traefik Gateway Module

Deploys [Traefik](https://traefik.io/) as an ingress controller using Helm and
creates a cert-manager ClusterIssuer for Automatic Certificate Management
Environment (ACME) certificate management with Cloudflare DNS-01 challenge
validation.

## Prerequisites

- A Kubernetes cluster with cluster-admin access
- [cert-manager](https://cert-manager.io/) installed in the cluster
- A Kubernetes Secret containing a Cloudflare API token with DNS edit permissions
- OpenTofu >= 1.6.0

## Usage

```hcl
module "traefik" {
  source = "path/to/modules/traefik"

  namespace                        = "traefik"
  acme_email                       = "admin@example.com"
  cloudflare_api_token_secret_name = "cloudflare-api-token"

  # Optional: use staging for testing
  # acme_server = "https://acme-staging-v02.api.letsencrypt.org/directory"
}
```

## Inputs

| Name | Description | Type | Default | Required |
|------|-------------|------|---------|----------|
| `namespace` | Namespace where Traefik will be installed | `string` | `"traefik"` | no |
| `create_namespace` | Whether the module should create the namespace | `bool` | `true` | no |
| `chart_repository` | Helm repository hosting the Traefik chart | `string` | `"https://traefik.github.io/charts"` | no |
| `chart_name` | Name of the Helm chart | `string` | `"traefik"` | no |
| `chart_version` | Traefik Helm chart version | `string` | `"25.0.3"` | no |
| `helm_release_name` | Name assigned to the Helm release | `string` | `"traefik"` | no |
| `helm_wait` | Whether to wait for the release to succeed | `bool` | `true` | no |
| `helm_timeout` | Timeout in seconds for Helm operations | `number` | `600` | no |
| `helm_values` | Inline YAML values for the Helm release | `list(string)` | `[]` | no |
| `helm_values_files` | Paths to YAML files with Helm values | `list(string)` | `[]` | no |
| `service_type` | Kubernetes service type | `string` | `"LoadBalancer"` | no |
| `external_traffic_policy` | Traffic policy (Local preserves client IPs) | `string` | `"Local"` | no |
| `ingress_class_name` | Name of the IngressClass | `string` | `"traefik"` | no |
| `ingress_class_default` | Set as default IngressClass | `bool` | `false` | no |
| `dashboard_enabled` | Enable the Traefik dashboard | `bool` | `false` | no |
| `dashboard_hostname` | Hostname for dashboard IngressRoute | `string` | `null` | no |
| `http_to_https_redirect` | Redirect HTTP to HTTPS | `bool` | `true` | no |
| `prometheus_metrics_enabled` | Enable Prometheus metrics | `bool` | `true` | no |
| `service_monitor_enabled` | Create ServiceMonitor for Prometheus Operator | `bool` | `true` | no |
| `tolerations` | Tolerations for pod scheduling | `list(object)` | CriticalAddonsOnly | no |
| `acme_email` | Email for ACME registration | `string` | - | **yes** |
| `acme_server` | ACME server URL | `string` | Let's Encrypt production | no |
| `cluster_issuer_name` | Name of the ClusterIssuer resource | `string` | `"letsencrypt-prod"` | no |
| `cloudflare_api_token_secret_name` | Kubernetes secret with Cloudflare token | `string` | - | **yes** |
| `cloudflare_api_token_secret_key` | Key in the Cloudflare token secret | `string` | `"token"` | no |

## Outputs

| Name | Description |
|------|-------------|
| `namespace` | Namespace where Traefik is installed |
| `helm_release_name` | Name of the Traefik Helm release |
| `cluster_issuer_name` | Name of the ClusterIssuer |
| `cluster_issuer_ref` | Reference object for use in Certificate resources |
| `dashboard_hostname` | Dashboard hostname (null if disabled) |
| `ingress_class_name` | Name of the IngressClass |

## Dashboard Security

The Traefik dashboard is disabled by default for security. If enabled,
ensure proper access controls are configured:

```hcl
module "traefik" {
  source = "path/to/modules/traefik"

  dashboard_enabled  = true
  dashboard_hostname = "traefik.internal.example.com"

  # Additional security via Helm values
  helm_values = [<<-YAML
    dashboard:
      enabled: true
    # Consider adding middleware for authentication
    YAML
  ]
}
```

## ACME Staging

For testing, use the Let's Encrypt staging server to avoid rate limits:

```hcl
module "traefik" {
  source = "path/to/modules/traefik"

  acme_server         = "https://acme-staging-v02.api.letsencrypt.org/directory"
  cluster_issuer_name = "letsencrypt-staging"
  # ...
}
```

Note: Staging certificates are not trusted by browsers.

## Cloudflare API Token

Create a Kubernetes secret containing a Cloudflare API token:

```bash
kubectl create secret generic cloudflare-api-token \
  --namespace cert-manager \
  --from-literal=token=<cloudflare-api-token>
```

The token requires the following permissions:

- Zone: DNS: Edit (for the zones intended for certificate issuance)
- Zone: Zone: Read (to list zones)

## Resources Created

1. **kubernetes_namespace.traefik** - Namespace (when `create_namespace = true`)
2. **helm_release.traefik** - Traefik Helm chart deployment
3. **kubernetes_manifest.cluster_issuer** - cert-manager ClusterIssuer

## Integration with Other Modules

Use the outputs to configure other resources:

```hcl
# Certificate for your application
resource "kubernetes_manifest" "certificate" {
  manifest = {
    apiVersion = "cert-manager.io/v1"
    kind       = "Certificate"
    metadata = {
      name      = "my-app-tls"
      namespace = "my-app"
    }
    spec = {
      secretName = "my-app-tls"
      issuerRef  = module.traefik.cluster_issuer_ref
      dnsNames   = ["app.example.com"]
    }
  }
}

# Ingress using Traefik's IngressClass
resource "kubernetes_ingress_v1" "my_app" {
  metadata {
    name      = "my-app"
    namespace = "my-app"
  }
  spec {
    ingress_class_name = module.traefik.ingress_class_name
    # ...
  }
}
```
