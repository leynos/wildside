# Local Kubernetes preview and Nile Valley integration design

## Purpose

Wildside ships the application artefacts that Nile Valley preview and GitOps
workflows consume: the backend container image, Helm chart, and health
contract. Nile Valley remains responsible for shared cluster automation,
environment overlays, and cross-application GitOps reconciliation. This
repository owns a developer-focused local preview loop that proves the
Wildside chart can install into a small local Kubernetes cluster. The default
mode remains Docker plus `k3d`; contributors on rootless Podman hosts can use
Podman plus `kind`.

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

`make local-k8s-up` validates required tools, creates or reuses the selected
local cluster, creates a generated Kubernetes Secret for the release-mode
session signing key, builds the backend image with the selected container
engine, imports it into the cluster, and installs or upgrades the Helm release
with the local values file.

Docker plus `k3d` is the default because it preserves the original local
preview behaviour. Its ingress load balancer binds
`127.0.0.1:${WILDSIDE_K8S_PORT:-8088}` to avoid exposing the preview outside
the developer machine:

```bash
make local-k8s-up
```

Rootless Podman plus `kind` is selected with provider-neutral environment
variables:

```bash
WILDSIDE_CONTAINER_ENGINE=podman WILDSIDE_K8S_PROVIDER=kind make local-k8s-up
```

In this mode, the helper creates the `kind` cluster through a delegated user
scope and pins the node image to a Kubernetes version accepted by the Helm
chart's `kubeVersion` constraint:

```bash
systemd-run --scope --user -p Delegate=yes env KIND_EXPERIMENTAL_PROVIDER=podman kind create cluster
```

Podman stores unqualified image names under `localhost/`, while Kubernetes
resolves an unqualified image such as `wildside-backend:local` as
`docker.io/library/wildside-backend:local`. The helper tags the archive export
with Docker's implicit registry name before `kind load image-archive`, so the
image already present on the node matches the pod spec. Each import uses a
unique temporary archive path and removes it after success or failure, so
parallel preview imports do not contend for one shared file.

The `kind` cluster deliberately has no host-port mapping. After `up` or
`status`, run the printed `kubectl port-forward` command to open the preview
locally. With default names, the command is:

```bash
kubectl --context kind-wildside-preview --namespace wildside port-forward svc/wildside 8088:80
```

If `WILDSIDE_HELM_RELEASE` changes the release name, the service name in the
printed command follows Helm's fullname rule.

Required executables table:

| Mode                  | Required executables                                           |
| --------------------- | -------------------------------------------------------------- |
| Docker plus `k3d`     | `docker`, `helm`, `k3d`, `kubectl`, and `uv`                   |
| Docker plus `kind`    | `docker`, `helm`, `kind`, `kubectl`, and `uv`                  |
| Podman plus `kind`    | `podman`, `helm`, `kind`, `kubectl`, `systemd-run`, and `uv`   |
| Podman plus `k3d`[^1] | `podman`, `helm`, `k3d`, `kubectl`, and `uv`                   |

[^1]: Podman plus `k3d` is accepted by the configuration surface because image
    builds use the selected container engine and cluster lifecycle uses the
    selected Kubernetes provider. The primary rootless path is Podman plus
    `kind`.

Configuration variables table:

| Variable                    | Default                  | Purpose                                      |
| --------------------------- | ------------------------ | -------------------------------------------- |
| `WILDSIDE_CONTAINER_ENGINE` | `docker`                 | Container engine: `docker` or `podman`.      |
| `WILDSIDE_K8S_PROVIDER`     | `k3d`                    | Local cluster provider: `k3d` or `kind`.     |
| `WILDSIDE_K8S_CLUSTER`      | `wildside-preview`       | Local cluster name.                          |
| `WILDSIDE_K8S_PORT`         | `8088`                   | Loopback or port-forward preview port.       |
| `WILDSIDE_K8S_NAMESPACE`    | `wildside`               | Kubernetes namespace.                        |
| `WILDSIDE_HELM_RELEASE`     | `wildside`               | Helm release name.                           |
| `WILDSIDE_IMAGE`            | `wildside-backend:local` | Local image reference.                       |
| `WILDSIDE_KIND_NODE_IMAGE`  | `kindest/node:v1.31.0`   | `kind` node image.                           |

`WILDSIDE_IMAGE` must include a tag because the Helm chart receives repository
and tag as separate values. `WILDSIDE_K3D_CLUSTER` and `WILDSIDE_K3D_PORT`
remain backwards-compatible aliases when the provider-neutral cluster and port
variables are unset.

Keep `WILDSIDE_KIND_NODE_IMAGE` within the chart's supported Kubernetes range,
currently `>=1.26.0-0 <1.32.0-0`. Leaving it unset uses Kubernetes `v1.31.0`.

The local values file enables `sessionSecret` and sets
`SESSION_KEY_FILE=/var/run/secrets/wildside-session/session_key`. The helper
creates the `wildside-session-key` Secret when missing, treats concurrent
already-exists responses as reuse, and reuses existing key material on later
`up` runs. This avoids committed secret material while keeping the release-mode
session configuration path.

The kube context name is derived as `{provider}-{cluster}`. For the default
cluster, this is `k3d-wildside-preview`; for the rootless Podman plus `kind`
path, this is `kind-wildside-preview`.

## Validation

The local preview helper has unit coverage for configuration parsing,
preflight validation, image reference parsing, provider-aware cluster
lifecycle commands, provider-aware image import commands, status, logs, and
port-forward output. Full end-to-end preview validation requires the selected
container engine, selected Kubernetes provider, `kubectl`, Helm, and an
available loopback port. Rootless Podman plus `kind` also requires working
user-level systemd scopes with cgroup delegation.

If those tools are absent, the CLI must fail early with a clear
missing-executable message rather than partially creating infrastructure.

Repository-wide validation remains:

```bash
make check-fmt
make lint
make audit
make test
```
