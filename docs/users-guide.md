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

| Variable                    | Default                  | Description                                                                                                |
| --------------------------- | ------------------------ | ---------------------------------------------------------------------------------------------------------- |
| `WILDSIDE_CONTAINER_ENGINE` | `docker`                 | Container engine used for local image builds and imports. Set to `podman` for the rootless `kind` flow.    |
| `WILDSIDE_K8S_PROVIDER`     | `k3d`                    | Local Kubernetes provider. Use `k3d` for the default Docker-backed preview, or `kind` for rootless Podman. |
| `WILDSIDE_K8S_CLUSTER`      | `wildside-preview`       | Provider-neutral cluster name. Overrides the legacy `WILDSIDE_K3D_CLUSTER` alias when both are set.        |
| `WILDSIDE_K8S_PORT`         | `8088`                   | Host-port ingress binding for the `k3d` flow only. The `kind` flow uses `kubectl port-forward` instead.    |
| `WILDSIDE_K8S_NAMESPACE`    | `wildside`               | Kubernetes namespace used by Helm, kubectl, and the local session Secret.                                  |
| `WILDSIDE_HELM_RELEASE`     | `wildside`               | Helm release name for the local preview chart install.                                                     |
| `WILDSIDE_IMAGE`            | `wildside-backend:local` | Tagged backend image name built, imported, and passed to Helm.                                             |

`WILDSIDE_K3D_CLUSTER` and `WILDSIDE_K3D_PORT` remain legacy aliases when the
provider-neutral names are unset. `WILDSIDE_K3D_PORT` follows
`WILDSIDE_K8S_PORT` and is used only for the `k3d` host-port ingress binding.

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

The local session key is generated when missing, created as the
`wildside-session-key` Secret, reused on later deploys, and mounted by the
chart at `/var/run/secrets/wildside-session/session_key`.

Nile Valley owns shared preview and GitOps automation. The local preview in
this repository is for developer validation of the Wildside chart and runtime
contract.

This guide records user-visible server behaviour for Wildside application
programming interface (API) consumers. It focuses on contracts that clients can
rely on when calling the backend.

## Submit a route request

`POST /api/v1/routes` accepts an authenticated route-generation request. Send a
valid session; requests without one return `401 Unauthorized`.

Each of `origin` and `destination` must be either an identifier string, such as
`"saved:home"` or `"poi:work"`, or a coordinate object:

```json
{
  "origin": {"lat": 51.5, "lng": -0.1},
  "destination": "poi:work",
  "preferences": {
    "mode": "walking",
    "themes": ["heritage"],
    "themeIds": ["theme-1"],
    "interestThemeIds": ["interest-1"],
    "avoid": ["busy-roads"],
    "avoidStairs": true
  }
}
```

Coordinates use WGS84 decimal degrees. Latitude must be between `-90` and `90`,
and longitude between `-180` and `180`, inclusive. The coordinate object must
contain only `lat` and `lng`; both values are required. The optional
`preferences` object accepts these fields, all of which are optional:

| Field              | Type     | Meaning                     |
| ------------------ | -------- | --------------------------- |
| `mode`             | string   | Routing mode.               |
| `themes`           | string[] | Theme names.                |
| `themeIds`         | string[] | Theme identifiers.          |
| `interestThemeIds` | string[] | Interest-theme identifiers. |
| `avoid`            | string[] | Route features to avoid.    |
| `avoidStairs`      | boolean  | Whether to avoid stairs.    |

Each preference list (`themes`, `themeIds`, `interestThemeIds`, and `avoid`)
may contain at most 64 entries. Each string preference value, including `mode`
and every list entry, may contain at most 64 UTF-8 bytes. Values or lists
exceeding these limits are rejected as `400 Bad Request`.

The request body rejects null `origin`, `destination`, or `preferences`,
boolean or array locations, coordinate objects with missing or unknown fields,
and unknown fields at the top level or in `preferences`. Omit `preferences`
when no preferences are needed; do not send it as `null`. These validation
failures return `400 Bad Request`.

Successful submissions return `202 Accepted` with a generated request ID:

```json
{
  "requestId": "<request-id>",
  "status": "accepted"
}
```

To make retries safe, send an `Idempotency-Key` header containing a UUID. The
first request returns `202 Accepted` with `status: "accepted"`. Repeating the
same key with the same payload returns `202 Accepted` with the original
`requestId` and `status: "replayed"`. Reusing the key with a different payload
returns `409 Conflict`. An invalid or empty key returns `400 Bad Request`; the
header is optional. If a backend service required to accept or persist the
submission is unavailable, the endpoint returns `503 Service Unavailable`.

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
