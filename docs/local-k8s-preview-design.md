# Local k3d preview and Nile Valley integration design

## Purpose

Wildside ships the application artefacts that Nile Valley preview and GitOps
workflows consume: the backend container image, Helm chart, and health
contract. Nile Valley remains responsible for shared cluster automation,
environment overlays, and cross-application GitOps reconciliation. This
repository owns a developer-focused local preview loop that proves the Wildside
chart can install into a small `k3d` cluster.

## Runtime health contract

The backend binary is the production runtime entry point. It starts the Actix
Web server and exposes two unauthenticated health endpoints:

- `GET /health/live` reports process liveness.
- `GET /health/ready` reports whether startup completed and the service is
  ready for traffic.

Health semantics live in the domain layer through `ProcessHealth` and the
`HealthObserver` port. The HTTP adapter only maps domain observations to HTTP
status codes, cache headers, and a small JSON health envelope. This keeps
Kubernetes, Actix Web, and other transport concerns outside the domain.

## Container image contract

The backend image is built by `deploy/docker/backend.Dockerfile`. It uses a
multi-stage Rust build and a Debian slim runtime with explicit runtime
libraries. The runtime process runs as a non-root user and defaults to
`HOST=0.0.0.0` and `PORT=8080`.

The image health check probes `/health/live` on the configured port. Kubernetes
readiness remains a chart concern and probes `/health/ready`.

## Helm chart contract

The Wildside chart under `deploy/charts/wildside` is the deployment interface
consumed by Nile Valley. It renders:

- a Deployment and Service for the backend;
- a ConfigMap for non-secret environment values;
- Secret-derived environment variables through `secretEnvFromKeys`;
- optional `ExternalSecret` resources for external-secrets operators;
- optional service accounts, ingress, autoscaling, and disruption budgets.

Secret validation is opt-in through `validateExistingSecret` so GitOps and
offline `helm template` runs do not require live cluster access. When
`externalSecret.enabled` is true and `existingSecretName` is unset, the
ExternalSecret target name becomes the effective Secret name used by the
Deployment.

Use `deploy/charts/wildside/values.local.yaml` for the local preview. Nile
Valley should provide environment-specific values in its GitOps overlays.

## Local preview workflow

The local preview CLI is `scripts/local_k8s.py`. It uses `uv` inline script
metadata and a Cyclopts command surface:

```bash
make local-k8s-up
make local-k8s-status
make local-k8s-logs
make local-k8s-down
```

`make local-k8s-up` validates required tools, creates or reuses the `k3d`
cluster, builds the backend image, imports it into the cluster, and installs or
upgrades the Helm release with the local values file. The ingress load balancer
is bound to `127.0.0.1` to avoid exposing the preview outside the developer
machine.

The workflow expects these executables on `PATH`:

- `docker`
- `helm`
- `k3d`
- `kubectl`
- `uv`

Configuration can be overridden with environment variables:

| Variable                 | Default                  | Purpose                |
| ------------------------ | ------------------------ | ---------------------- |
| `WILDSIDE_K3D_CLUSTER`   | `wildside-preview`       | k3d cluster name.      |
| `WILDSIDE_K3D_PORT`      | `8088`                   | Loopback ingress port. |
| `WILDSIDE_K8S_NAMESPACE` | `wildside`               | Kubernetes namespace.  |
| `WILDSIDE_HELM_RELEASE`  | `wildside`               | Helm release name.     |
| `WILDSIDE_IMAGE`         | `wildside-backend:local` | Local image reference. |

`WILDSIDE_IMAGE` must include a tag because the Helm chart receives repository
and tag as separate values.

## Validation

The local preview helper has unit coverage for preflight validation and image
reference parsing. Full end-to-end preview validation requires Docker, `k3d`,
`kubectl`, Helm, and an available loopback port. If those tools are absent, the
CLI must fail early with a clear missing-executable message rather than
partially creating infrastructure.

Repository-wide validation remains:

```bash
make check-fmt
make lint
make test
```
