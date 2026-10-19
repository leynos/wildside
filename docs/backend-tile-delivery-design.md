# Backend tile delivery design

## 1. Scope and status

Audience: backend developers and platform engineers. This document specifies
planned backend roadmap section 6.1. It does not claim that Martin, route
inference workers, tile authorization, or dashboards are implemented.
[ADR 003](adr-003-protected-route-tile-delivery.md) records the proposed
authorization boundary. The backend owns delivery adapters; the
`wildside-engine` repository owns reusable ingestion and route inference.

Wildside needs public POI tiles and access-controlled route tiles derived from
its own data, without coupling domain services to delivery technology. The
request flow is ingress → backend grant validation for routes → Martin →
PostGIS; public POIs bypass grant validation. Route inference runs separately
in workers and materializes geometry before any tile can be requested.

## 2. Current state and dependencies

The chart in `deploy/charts/wildside` currently deploys only the backend. Its
`networking.k8s.io/v1` Ingress sends every configured path to that Service.
There is no Martin Deployment, source configuration, or monitoring resource.

The schema baseline migration stores `pois.location` as native PostgreSQL
`POINT` and `routes.path` as native `PATH`; neither is PostGIS geometry.
`route_pois` stores ordered membership using `(element_type, id)` identities.
The existing GiST point index does not provide a PostGIS geometry contract.
There is no `get_route_tile` function. Do not expose the default legacy path as
an inferred walking route.

As inspected on 19 October 2026, the backend pins `wildside-data` at
`2db6cbfb8ec68b611bc74fef6504f044c377b851` for ingestion; it does not depend on
the `wildside-engine` facade. The engine evidence below refers to upstream
commit `de1e6c6c4989fe6fa8070814d4f853c1a7adf8e9`, not that older backend pin:

- The [facade][engine-facade] exports `Solver`, `SolveRequest`, `SolveResponse`,
  `Route`, `PoiStore`, and optional `VrpSolver` and SQLite store
  implementations.
- [SolveRequest][engine-solver] includes start, optional end, duration in
  minutes, interests, seed, and optional maximum candidate count. `Solver` is
  synchronous; results contain a route, score, and diagnostics. Its errors are
  currently only `InvalidRequest` and `NotImplemented`.
- [Route][engine-route] contains endpoints, ordered POIs, and total duration;
  it has no routed line or per-leg geometry. A travel-time matrix does not
  supply the paths that produced its durations.
- [PointOfInterest][engine-poi] has a `u64` ID and WGS84 coordinates, but no
  typed OpenStreetMap (OSM) element identity. Backend composite identifiers
  cannot safely be reduced to the numeric OSM ID alone.
- The [engine roadmap][engine-roadmap] already marks the native solver and
  travel-time provider complete. Its ingestion provenance, progress, and
  Postgres load steps remain open. Extend those entries rather than adding a
  competing ingestion pipeline.

## 3. Ownership and backend integration

Keep framework, engine-crate, database, JWT, and exporter types outside the
backend domain. Reuse `RouteQueue`, `RouteRepository`, `RouteCache`, and the
existing event/metrics ports. Introduce a narrow route-inference port only for
the missing solver boundary: backend-owned request, inferred route, and typed
failure values. Its scope is worker route computation, not tile rendering or
HTTP access. The implementation must confirm no equivalent port has landed
before adding it. A new tile repository abstraction is unnecessary if the
existing route repository can provide access and revision metadata.

An outbound engine adapter translates to `wildside_engine::SolveRequest`,
invokes `Solver::solve`, and maps the result back. Bootstrap selects the
solver, store, scorer, and travel-time provider explicitly. Pin the facade and
any companion engine packages to one tested revision and declare required
features; do not assume the facade re-exports every constructor or ingestion
API. Keep the existing `wildside-data` ingestion adapter and update its pin
with compatibility tests when the solver dependency is introduced.

Translate longitude to `x` and latitude to `y`; validate geographic bounds,
finite coordinates, minute conversion and overflow, themes, candidate limits,
and seed semantics. Resolve engine POI identities back to `(element_type, id)`
without collisions. Until the engine identity contract is available, a stable,
persisted bijection is required; hashing numeric OSM IDs is insufficient.
Version the input dataset, solver, and request contract in cache keys and
stored provenance so an ingestion refresh cannot reuse incompatible route
results.

Roadmap 5.3.1 must supply a real worker bootstrap, not merely an Apalis queue
adapter. Run synchronous inference outside Actix request threads with bounded
worker concurrency. A timeout must not silently release capacity while detached
solver work continues; retain capacity until completion, or use a killable
worker process until cooperative cancellation is supported. Classify invalid
input, no feasible route, provider failure, and worker failure without
unbounded retries. Backend queue policy owns retry and dead-letter decisions.

Persist the route, ordered POIs, geometry, revision, and provenance atomically
through the repository adapter. Use the request/job identity for idempotent
retries, and publish completion through event ports only after commit. Test
crashes before and after commit and duplicate job delivery. Never invoke the
solver on a tile request, and never substitute straight lines between POIs for
walkable inferred geometry. POI tiles can ship before this route capability.

## 4. PostGIS read models and function contract

Provision a PostGIS-capable database and install its extension through the
privileged migration path, never through Martin startup. Add indexed geometry
columns or dedicated read models, with a documented single writer and update
transaction. Build POI geometry explicitly from longitude and latitude as
`geometry(Point,4326)`; reject invalid coordinates and prove backfill parity.
Maintain it on ingestion updates. Use a curated view exposing only element
type, ID, name, popularity, and geometry; exclude narratives and private data.

Store verified inferred route geometry as `geometry(LineString,4326)` with
route UUID and immutable revision. Preserve legacy `PATH` while consumers
migrate; mark rows without inferred geometry unavailable for route tiles and
regenerate them rather than interpreting placeholder paths as valid output. Add
spatial indexes and test query plans against representative fixtures. Define
antimeridian splitting and Web Mercator latitude handling explicitly; reject
unsupported geometry until those cases have passing tests.

Use the documented [Martin function-source interface][martin-functions]:

```sql
get_route_tile(z integer, x integer, y integer, query_params json)
RETURNS bytea
```

Expose it as the explicitly configured internal source `routes`. The backend
constructs `/routes/{z}/{x}/{y}?route_id=<uuid>&revision=<revision>` only after
authorization. Martin passes query parameters as JSON; it does not infer an
extra route-ID path segment or forward an Authorization header into SQL.

The function validates required UUID/revision values and integer tile bounds
(`0 <= z <= 22`, `0 <= x,y < 2^z`) before spatial work. Restrict rows to the
requested route and revision, transform geometry to EPSG:3857, intersect with
`ST_TileEnvelope`, clip using `ST_AsMVTGeom`, and encode with `ST_AsMVT` into
layer `routes` with extent 4096 and a documented buffer. Match the spatial
prefilter to that buffer so boundary features are retained. Use parameterized
SQL, bounded statement timeouts, and no dynamic SQL from query parameters.

For an existing authorized route outside the tile envelope, return an empty MVT
response; freeze the pinned Martin version's exact status/content-type
behaviour in contract tests. Malformed requests return 400 at the backend;
missing, deleted, or unavailable routes return 404 before Martin is called.
Decode fixture MVT bytes to verify layers, attributes, coordinates, clipping,
empty tiles, and revision isolation. Test fresh and upgrade migrations.

Use a dedicated non-owner, non-superuser Martin role with `CONNECT`, schema
`USAGE`, `SELECT` on curated read models, and `EXECUTE` only on the approved
function. Revoke default `PUBLIC` function execution and schema creation. Prefer
`SECURITY INVOKER` with a fixed search path and schema-qualified objects; no
JWT keys or privileged function execution belong in the database. Prove the
role cannot write, alter schema, or read unrelated user tables. Database
credentials are infrastructure credentials, not end-user authorization.

## 5. JWT authorization and public requests

Add `POST /api/v1/routes/{route_id}/tile-grant` behind the existing session and
cross-site request forgery protection. Check current route access through the
repository-backed policy and return a short-lived JWT, expiry, and tile URL
template. Proposed claims are `sub`, `route_id`, `revision`, `iss`, `aud`,
`iat`, `nbf`, and `exp`; cap validity at five minutes with at most 30 seconds
of clock skew. Configure issuer and audience explicitly. Keep signing keys
separate from session-cookie keys and rotate by key ID with bounded overlap.

The public route endpoint is `/tiles/routes/{route_id}/{z}/{x}/{y}.pbf`, served
by a backend inbound adapter. Require `Authorization: Bearer <token>`; do not
put credentials in URLs. The adapter allowlists algorithms and key IDs,
verifies signature and all required claims, checks route/revision binding, and
rechecks current subject access and route availability on every request.
Missing or invalid tokens return 401; a valid token without current access
returns 403. Never trust a caller's subject header, duplicate query key,
arbitrary Martin source, or upstream URL.

After authorization, a bounded outbound HTTP adapter requests the fixed Martin
source using server-derived route/revision parameters. It forwards only safe
content headers, caps response size, and maps upstream timeout to 504 and
upstream failure to 502 without exposing SQL or credentials. Domain policy
returns access decisions; it neither parses JWTs nor proxies tile bytes.

Public POI tiles use `/tiles/pois/{z}/{x}/{y}.pbf`. Ingress strips `/tiles` and
any suffix only as required by the pinned Martin version; test this rewrite
instead of assuming native support for the public URL. TileJSON must advertise
public URLs. Any route TileJSON endpoint must enforce the same grant policy. For
`tiles.wildside.app`, retain the same path contract and configure exact
allowed web origins, Authorization preflight, TLS, and cache variation by
Origin. Cross-origin resource sharing (CORS) is not authorization.

Disable shared CDN, proxy, browser, and service-worker caching of private route
responses initially (`Cache-Control: private, no-store`). Authorize before
consulting any internal cache. Internal Martin caching may be enabled only when
tests establish route/revision isolation. Public POI caching uses dataset
revision invalidation. Test revoked access and expired tokens after warming all
enabled caches, including deletion and ownership changes.

## 6. Helm and network contract

Extend `deploy/charts/wildside` with opt-in `martin.enabled` and a separate
route-source switch. Keep the current backend-only rendering unchanged when
disabled. Use a pinned upstream Martin image and mounted configuration; no
custom image is needed merely to provide `config.yaml`.

Specify values and schema validation for image digest, replicas, resources,
port, pool limits, statement timeout, cache limits, source switches, existing
Secret/key references, probes, ingress mode, allowed origins, and monitoring.
Mount database credentials from Secrets or the existing ExternalSecret pattern;
never put connection passwords in a ConfigMap, rendered values, or logs. Keep
GitOps rendering offline and validate live Secret existence only when opted in.

Render a Martin Deployment, ClusterIP Service, non-secret ConfigMap, and
network policies. Verify non-root execution, read-only filesystem requirements,
health endpoints, and writable scratch needs against the chosen image.
Readiness must confirm configured sources are usable; liveness must not restart
every replica on a transient database outage. Reload configuration and
credentials through a controlled rollout with tested readiness and rollback
behaviour.

Use standard Kubernetes Ingress, matching the existing chart API. Create
explicit path backends: route requests reach the backend validator; POI
requests reach Martin. Do not publish a catch-all Martin host, source
catalogue, composite source URL, internal `routes` endpoint, or metrics
endpoint. Document controller-specific prefix rewriting in Nile Valley
overlays; do not require Traefik `IngressRoute` custom resources for the
default chart.

Network policies permit only the backend and ingress controller to reach
Martin, only the monitoring collector to scrape metrics, and only required
DNS/database egress. Ingress allowlisting must prevent the controller from
forwarding arbitrary paths to Martin: network policy alone cannot distinguish
public and protected sources. Require a policy-enforcing cluster network plugin
and verify direct-source denial in the target preview environment.

Roll out database extension/read models and role grants first, then workers and
route persistence, function and authorization, Martin, and finally ingress.
POI-only mode skips route dependencies. Helm render tests cover both ingress
modes, disabled resources, Secret references, route-source prerequisites, and
optional monitoring custom resources. Preview tests use real PostGIS and the
pinned Martin image, decode both tile sources, and exercise denied requests.
Rollback disables ingress and Martin before reverting compatible application
changes; retain additive geometry data and never drop PostGIS on rollback.

## 7. Observability and acceptance

Instrument the backend tile adapter and Martin independently. Reuse outbound
metrics adapters for domain observations; no Prometheus dependency enters the
domain or engine types. Engine diagnostics remain values, mapped through
backend metrics ports. Verify architecture lint enforces this boundary.

At implementation, capture the pinned Martin metrics endpoint and emitted names
in a fixture; do not assume every release exports `/metrics` or cache counters.
Use supported native metrics where present and adapter/ingress measurements for
missing request coverage. In particular, authorization failures never reach
Martin. Keep scrape endpoints private. Render ServiceMonitor and dashboard
ConfigMaps only when explicitly enabled and the relevant operator is present;
Nile Valley owns installation of the shared monitoring stack.

Provide version-controlled Grafana panels for request rate, p50/p95/p99
latency, 4xx/5xx ratios, authorization rejections, upstream timeouts, and
scrape health. Separate POI/route sources and backend end-to-end versus Martin
latency. Labels are bounded source, operation, and status class; never include
token, route ID, user ID, coordinates, or raw URL labels. Redact authorization
headers and query strings from access logs. Use trace IDs for correlation
without logging tokens.

Provisional rollout alerts are p95 tile latency above 500 ms or 5xx ratio above
1% for ten minutes with at least 100 requests in that window, and a missing
scrape target for five minutes. Record the load fixture and concurrency used,
then validate or revise these budgets before production enablement. Dashboard
and alert tests must inject failures, verify series and percentile units, and
prove firing and recovery. Link operational instructions for pool exhaustion,
key rotation, database outage, rejected grants, and disabling route ingress.
Update OpenAPI, the users' guide, and the local preview guide when the
endpoints ship; document MapLibre header injection and grant refresh before
expiry.

## 8. Proposed second PR: engine roadmap amendments

Proposed title: **Specify route geometry and ingestion identity contracts** in
`leynos/wildside-engine`. This is a separate documentation PR proposal, not an
opened PR or a claim that engine functionality has been delivered. Amend
`docs/roadmap.md` and the corresponding engine design chapters as follows:

1. Extend data foundation steps 1.2/1.3 and stable API step 2.3 with a typed,
   collision-free OSM element identity propagated through ingestion, SQLite
   artefacts, spatial stores, solver output, and serialization. Define
   migration compatibility for existing `u64` IDs. Acceptance: node and way
   with the same numeric ID survive an ingest/store/solve round trip as
   distinct POIs.
2. Extend steps 2.3 and 3.2 with a provider-neutral route-leg geometry contract
   and an implementation that resolves selected waypoints into walkable paths
   using the same travel profile as the matrix. Return WGS84 geometry, ordered
   legs, duration/distance units, and dataset/provider provenance. Cover start
   and end connectors, round trips, unreachable legs, and geometry failure;
   never return a straight-line fallback as a valid walking route. Acceptance:
   deterministic provider fixtures prove endpoint continuity, waypoint order,
   finite coordinates, and agreement with the time budget.
3. Extend stable API/testing steps 2.3/4.1 with typed no-solution and provider
   failures, retry classification, and a bounded-work/cancellation contract.
   Preserve seed determinism against a fixed dataset and document feature
   combinations and facade exports needed by library consumers. Acceptance:
   fixture provider failures remain distinguishable from invalid input, and
   execution-limit tests prove work stops or reports its non-cancellable state.
4. Refine existing open step 1.5 progress/provenance/Postgres-loading tasks into
   reusable versioned ingestion outputs, progress values or callbacks, and
   documented incremental/upsert semantics. Keep backend migrations, geofences,
   and application-owned PostgreSQL loading in Wildside; any generic engine
   exporter must not own Wildside's schema. Acceptance: reproducible artefacts
   preserve identity and provenance and support a backend-owned idempotent load.

The backend route implementation depends on items 1–3 (or an explicitly tested
backend identity bridge for item 1); POI-only Martin deployment does not. Item
4 refines existing ingestion work and is not a new tile prerequisite. Keep
Martin, MVT encoding, PostGIS tile functions, user access, JWT issuance, Helm,
and Grafana out of the engine PR. Link the engine PR and its eventual accepted
revision from backend task 6.1.1.2 before route implementation starts.

[engine-facade]: https://github.com/leynos/wildside-engine/blob/de1e6c6c4989fe6fa8070814d4f853c1a7adf8e9/src/lib.rs
[engine-solver]: https://github.com/leynos/wildside-engine/blob/de1e6c6c4989fe6fa8070814d4f853c1a7adf8e9/wildside-core/src/solver.rs
[engine-route]: https://github.com/leynos/wildside-engine/blob/de1e6c6c4989fe6fa8070814d4f853c1a7adf8e9/wildside-core/src/route.rs
[engine-poi]: https://github.com/leynos/wildside-engine/blob/de1e6c6c4989fe6fa8070814d4f853c1a7adf8e9/wildside-core/src/poi.rs
[engine-roadmap]: https://github.com/leynos/wildside-engine/blob/de1e6c6c4989fe6fa8070814d4f853c1a7adf8e9/docs/roadmap.md
[martin-functions]: https://maplibre.org/martin/sources-pg-functions/
