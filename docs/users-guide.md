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

### How long a key stays replayable

An idempotency key is replayable until its record expires.
`IDEMPOTENCY_TTL_HOURS` sets that window on the server, defaulting to 24 hours
and clamped to between one hour and ten years, so a value outside that range is
brought back to the nearest bound rather than rejected. Sending the same key
after the window has passed starts a new request rather than replaying the old
response.

The variable, its default, and its clamp are unchanged. Operators need do
nothing. Where the server reads it moved in the change that introduced the
environment-seam policy, from the domain layer to a configuration adapter; that
is an internal Rust API move with no deployment or request-level effect, and
the path mapping for in-repository callers is in
[developers' guide](developers-guide.md).

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

## Job retries and dead letters

Background jobs run through Apalis on PostgreSQL. The retry and dead-letter
ports are implemented and covered by the test suite, but composing them into a
production worker and operator command is deferred to item 5.3.1 of the
delivery plan, and the port currently has only test callers. The SQL runbook
linked below is therefore the available operator procedure, not a fallback from
a shipped console. Each queue has one retry policy, configured with
`JobRetryPolicyConfig`:

| Field             | Meaning                                                 |
| ----------------- | ------------------------------------------------------- |
| `max_attempts`    | Total deliveries allowed, including the first.          |
| `base_delay`      | Nominal delay before the first retry.                   |
| `multiplier`      | Growth factor applied to each successive nominal delay. |
| `max_delay`       | Upper bound on a nominal delay, applied before jitter.  |
| `jitter_fraction` | Symmetric fraction by which each delay is randomized.   |

The nominal delay for attempt `n` is `base_delay × multiplier^(n − 1)`, clamped
to `max_delay`. Jitter then scales the clamped value by a factor in
`1 − jitter_fraction ..= 1 + jitter_fraction`, so the delay actually scheduled
for the first retry is `min(base_delay, max_delay)` adjusted by up to
`±jitter_fraction`, and a positive sample can exceed `max_delay` by
`jitter_fraction × max_delay`. Because the clamp is applied before jitter, a
`base_delay` larger than `max_delay` does not raise the first delay: with
`base_delay = 10s`, `max_delay = 1s`, and zero jitter, the first retry waits
one second, not ten.

A retryable failure is scheduled durably: the wrapper writes the next `run_at`
while the worker still owns the row, then returns the error for Apalis to
acknowledge. Waiting for the delay does not occupy a worker slot. A terminal
failure records `wildside.retry.dead_reason` in the job's metadata instead of
scheduling another attempt, and the row becomes a dead letter.

A failure is terminal for one of two reasons, and the reason selects the
recovery an operator should consider:

- `fatal` means the failure is not retryable at all, so the first delivery
  dead-letters the row however much budget remains. Requeuing it will usually
  fail again unless the underlying cause has been fixed.
- `exhausted` means the attempt budget was spent. A retryable failure ends the
  row when the delivery count reaches either the policy's `max_attempts` or the
  `max_attempts` stored on the job, whichever is lower. Requeuing it restores a
  fresh budget.

`list_dead` reports these as `DeadReason::Fatal` and `DeadReason::Exhausted`.
Rows written before this reason was recorded, or with an unrecognized label,
list as `DeadReason::Unclassified` rather than being hidden, so an upgrade does
not strand old dead letters.

The wrapper persists nothing when it cannot prove the current worker owns the
row, for example when the task has no worker context or its worker disagrees
with the recorded lock owner. Such a delivery stays failed. When the policy
would have scheduled a retry it increments the retry scheduling-failure
counter; when the policy would have recorded a terminal failure it increments
none of the retry, dead-letter, or terminal-write-failure counters, because no
write was attempted. A missing or mismatched identity logs an error naming the
refused decision; that identity defect is not itself a retry signal.

A write that the database refuses, or that fails, is a different case from an
identity defect. A refused or failed retry write increments the retry
scheduling-failure counter. A refused or failed terminal write increments
`route_queue_terminal_write_failure_total`, labelled by the reason it would
have stored, and leaves the dead-letter counter unchanged because the row's
state did not change. Both log a warning whose `decision` field is
`schedule_failure` or `terminal_write_failure` respectively. Alert on the
terminal-write-failure counter as well as the warnings, because the dead-letter
counter alone will not show a terminal write that never applied.

The `DeadLetterQueue` port defines the operations a future operator command
will drive:

- `list_dead` returns the newest dead jobs first.
- `requeue` restores the queue's configured attempt budget and clears the
  dead-letter reason.
- `discard` permanently removes a dead job.

`requeue` validates the row state, attempt count, requeue counter, and integer
budget range. It also looks the row's job type up in the configured
`RequeueBudgets` and rejects a job whose queue has no budget, so a requeued row
always carries a budget this process knows about. Until the operator command
exists, the
[dead-letter runbook](runbooks/queue-retry-and-dead-letter-operations.md) is
the current operator procedure. Its SQL does not pass through that guard:
checking the queue name and its current configured budget is the operator's
precondition there.

### Configuring the Apalis provider

`ApalisPostgresProvider::new` takes a `sqlx::PgPool`, the queue name, and the
queue's `JobRetryPolicy`. It is `async` and returns
`Result<Self, JobDispatchError>`. It derives the Apalis attempt budget from
`policy.max_attempts()`, so the `max_attempts` stored on each new job comes
from the policy rather than from a second value that could drift. Pass the same
policy to `RetryingJobServiceConfig::new` so the wrapper and the stored budget
agree. A policy limit that does not fit Apalis's signed `integer` attempt
column is rejected with `JobDispatchError::Rejected`.

The constructor gained the queue name and policy arguments when durable retries
were introduced. There are no compatibility shims or deprecated overloads
because the crate is pre-1.0, so an existing caller must pass both:

```rust
// Before durable retries: the provider set up the job table on its own.
let provider = ApalisPostgresProvider::new(pool.clone()).await?;

// After: the queue name and its policy supply the stored attempt budget.
let provider =
    ApalisPostgresProvider::new(pool.clone(), "route.generate", policy.clone()).await?;
```

Changing the policy does not rewrite rows already in the table: each stored job
keeps the `max_attempts` recorded when it was enqueued, and the new budget
applies to jobs enqueued afterwards. The retry wrapper still applies the
current policy on each failed delivery, so an older row with a larger stored
cap is dead-lettered once the policy's limit is reached; when that terminating
attempt is below the stored cap, the cap is lowered to it. A row whose stored
cap is below the current policy limit dead-letters at its own cap instead, so
raising the policy does not revive a row that was enqueued with a smaller
budget.

Retry jitter comes from a `JitterSource`. Use
`RetryingJobServiceConfig::with_jitter` to replace the default random source
with a deterministic one, which makes scheduled delays reproducible in a test.
