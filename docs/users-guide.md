# Wildside users' guide

## Backend health endpoints

The backend exposes unauthenticated health probes for operators and local
preview tooling:

- `GET /health/live` returns `200 OK` while the process is live.
- `GET /health/ready` returns `200 OK` after startup completes and `503
  Service Unavailable` while the process is not ready.

Both endpoints send `Cache-Control: no-store` and a JSON body with a top-level
`status` field (`pass` or `fail`) plus a `checks` object keyed by `liveness` or
`readiness`.

## Local k3d preview

Developers can run a local Kubernetes preview when Docker, Helm, `k3d`,
`kubectl`, and `uv` are installed:

```bash
make local-k8s-up
make local-k8s-status
make local-k8s-logs
make local-k8s-down
```

The default preview is reachable through loopback ingress at
`http://127.0.0.1:8088`. Override the port with `WILDSIDE_K3D_PORT` if that
port is already in use.

Useful overrides:

| Variable                    | Default                  |
| --------------------------- | ------------------------ |
| `WILDSIDE_K3D_CLUSTER`      | `wildside-preview`       |
| `WILDSIDE_K3D_PORT`         | `8088`                   |
| `WILDSIDE_K8S_NAMESPACE`    | `wildside`               |
| `WILDSIDE_HELM_RELEASE`     | `wildside`               |
| `WILDSIDE_IMAGE`            | `wildside-backend:local` |

`WILDSIDE_IMAGE` must include a tag. The preview helper splits the value into
the Helm chart's `image.repository` and `image.tag` settings.

Nile Valley owns shared preview and GitOps automation. The local preview in
this repository is for developer validation of the Wildside chart and runtime
contract.
