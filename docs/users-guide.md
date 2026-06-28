# Wildside server users guide

## Backend health endpoints

The backend exposes unauthenticated health probes for operators and local
preview tooling:

- `GET /health/live` returns `200 OK` while the process is live.
- `GET /health/ready` returns `200 OK` after startup completes and
  `503 Service Unavailable` while the process is not ready.

Both endpoints send `Cache-Control: no-store` and a JSON body with a top-level
`status` field (`pass` or `fail`) plus a `checks` object keyed by `liveness` or
`readiness`.

## Local Kubernetes preview

Developers can run a local Kubernetes preview when a supported container
engine, Kubernetes provider, Helm, `kubectl`, and `uv` are installed:

```bash
make local-k8s-up
make local-k8s-status
make local-k8s-logs
make local-k8s-down
```

The default preview uses Docker plus `k3d`, builds the backend image, imports
it into the local cluster, applies a runtime session signing Secret, and
installs the Wildside Helm chart. It is reachable through loopback ingress at
`http://127.0.0.1:8088`.

Rootless Podman users can use kind instead:

```bash
WILDSIDE_CONTAINER_ENGINE=podman WILDSIDE_K8S_PROVIDER=kind make local-k8s-up
```

The kind path creates the cluster without host-port mappings. Use
`make local-k8s-status` after deployment to print the provider-specific kube
context and the `kubectl port-forward` command for the Helm service.

Useful overrides:

| Variable                    | Default                  |
| --------------------------- | ------------------------ |
| `WILDSIDE_CONTAINER_ENGINE` | `docker`                 |
| `WILDSIDE_K8S_PROVIDER`     | `k3d`                    |
| `WILDSIDE_K8S_CLUSTER`      | `wildside-preview`       |
| `WILDSIDE_K8S_PORT`         | `8088`                   |
| `WILDSIDE_K8S_NAMESPACE`    | `wildside`               |
| `WILDSIDE_HELM_RELEASE`     | `wildside`               |
| `WILDSIDE_IMAGE`            | `wildside-backend:local` |

`WILDSIDE_K3D_CLUSTER` and `WILDSIDE_K3D_PORT` remain legacy aliases when the
provider-neutral names are unset.

Set `WILDSIDE_KIND_NODE_IMAGE` only when testing a different kind node image.
The default is `kindest/node:v1.31.0`, which satisfies the chart's Kubernetes
version range.

`WILDSIDE_IMAGE` must include a tag. The preview helper splits the value into
the Helm chart's `image.repository` and `image.tag` settings. For rootless
Podman plus kind, the helper saves the image to a temporary archive and loads
that archive into kind using the image name Kubernetes will pull.

Kube contexts are named `{provider}-{cluster}`. The default context is
`k3d-wildside-preview`; Podman plus kind with the default cluster uses
`kind-wildside-preview`.

The local session key is generated when missing, applied as the
`wildside-session-key` Secret, reused on later deploys, and mounted by the
chart at `/var/run/secrets/wildside-session/session_key`.

Nile Valley owns shared preview and GitOps automation. The local preview in
this repository is for developer validation of the Wildside chart and runtime
contract.

This guide records user-visible server behaviour for Wildside application
programming interface (API) consumers. It focuses on contracts that clients can
rely on when calling the backend.

## Users list pagination

`GET /api/v1/users` returns a paginated user-list response. Clients should
follow the `links.next` and `links.prev` URLs returned by the server instead of
building cursor values themselves.

The endpoint accepts:

- `cursor`: an opaque base64url cursor returned by a previous user-list
  response.
- `limit`: page size. The shared pagination default is 20 and the maximum is
  100.

Successful responses include the existing paginated envelope:

```json
{
  "data": [],
  "limit": 20,
  "links": {
    "self": "/api/v1/users",
    "next": null,
    "prev": null
  }
}
```

Pagination input errors use the standard Wildside error envelope and return
Hypertext Transfer Protocol (HTTP) `400 Bad Request`:

| Condition                              | Message                           | `details.field` | `details.code`          |
| -------------------------------------- | --------------------------------- | --------------- | ----------------------- |
| Cursor text is not a valid user cursor | `cursor is invalid`               | `cursor`        | `invalid_cursor`        |
| Cursor direction is not supported      | `cursor direction is unsupported` | `cursor`        | `unsupported_direction` |

Authentication and infrastructure errors keep their existing meanings.
Unauthenticated requests return `401`, repository availability failures return
`503`, and unexpected persistence query failures return a redacted `500`
response.
