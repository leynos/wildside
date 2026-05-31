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

# Wildside server users guide

This guide records user-visible server behaviour for Wildside API consumers.
It focuses on contracts that clients can rely on when calling the backend.


## Users list pagination

`GET /api/v1/users` returns a paginated user-list response. Clients should follow
the `links.next` and `links.prev` URLs returned by the server instead of
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
HTTP `400 Bad Request`:

| Condition                                  | Message                             | `details.field` | `details.code`          |
|--------------------------------------------|-------------------------------------|-----------------|-------------------------|
| Cursor text is not a valid user cursor    | `cursor is invalid`                 | `cursor`        | `invalid_cursor`        |
| Cursor direction is not supported          | `cursor direction is unsupported`   | `cursor`        | `unsupported_direction` |

Authentication and infrastructure errors keep their existing meanings.
Unauthenticated requests return `401`, repository availability failures return
`503`, and unexpected persistence query failures return a redacted `500`
response.
