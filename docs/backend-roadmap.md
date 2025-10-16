# Wildside backend roadmap

This roadmap captures the outstanding delivery work for the Wildside backend.
It follows the phase → step → task hierarchy defined in
`docs/documentation-style-guide.md`. Progress is tracked with checkboxes; tasks
may only be marked complete when their acceptance criteria are met and the
related automated checks pass.

## Phase 1 – Core access and sessions

### Step: Session lifecycle hardening

- [x] Implement `POST /api/v1/login` with signed-cookie sessions, two-hour TTL,
  and production-grade key management.
- [ ] Wire `/api/v1/users/me` and `/api/v1/users/me/interests` to require the
  session middleware, returning `401` with trace identifiers when unauthenticated.
- [ ] Enforce configuration toggles for `SESSION_SAMESITE`, `SESSION_COOKIE_SECURE`,
  and `SESSION_ALLOW_EPHEMERAL`, failing fast in release builds when secrets
  are missing or keys are too short.
- [ ] Document and script the rotation procedure for session signing keys,
  including Kubernetes secret rollout and dual validation during deploys.

### Step: Route submission idempotency

- [ ] Persist `Idempotency-Key` headers for `POST /api/v1/routes` in PostgreSQL,
  rejecting conflicting payloads with `409` and replaying successful responses
  within 24 hours.
- [ ] Capture idempotency audit metrics (hits, misses, conflicts) and expose them
  via Prometheus with labels for user scope and key age buckets.

## Phase 2 – Data platform foundation

### Step: Schema baseline

- [ ] Deliver Diesel migrations that materialise the schema in
  `docs/wildside-backend-architecture.md`, including GiST/GIN indices and unique
  constraints for composite keys.
- [ ] Generate ER-diagram snapshots from migrations and store them alongside
  documentation for traceability.

### Step: Data ingestion and enrichment

- [ ] Ship the Rust-based `ingest-osm` CLI with documentation covering filters,
  provenance logging, and deterministic reruns over launch geofences.
- [ ] Add Overpass enrichment workers with semaphore-governed quotas, circuit
  breaking, and metrics wired to the enrichment job counters.
- [ ] Configure enrichment provenance persistence (source URL, timestamp, bbox)
  and expose it via admin reporting endpoints.

## Phase 3 – Pagination infrastructure

### Step: Pagination crate foundation

- [ ] Implement `backend/crates/pagination` providing opaque cursor encoding,
  `PageParams`, and `Paginated<T>` envelopes with navigation links, backed by
  unit tests for cursor round-tripping.
- [ ] Add support for direction-aware cursors (`Next`/`Prev`) with serde-based
  encoding and property tests ensuring decode-encode stability.
- [ ] Publish crate-level documentation outlining ordering requirements,
  default/maximum limits (20/100), and error mapping guidelines.

### Step: Endpoint adoption

- [ ] Replace offset pagination in `GET /api/users` with the new crate, including
  Diesel filters that respect `(created_at, id)` ordering and bb8 connection
  pooling.
- [ ] Update the repository layer to surface pagination-aware errors (e.g. invalid
  cursor format, unsupported direction) with HTTP 400 responses.
- [ ] Ensure pagination telemetry records page size, cursor direction, and page
  traversal counts for analytics.

### Step: Documentation and quality gates

- [ ] Update the OpenAPI schema, async API artefacts, and developer guides to
  document the new `cursor`/`limit` query parameters and response envelope.
- [ ] Add integration tests exercising forward and backward pagination, plus
  contract tests guaranteeing link generation and page-size guardrails.
- [ ] Provide sample client implementations (TypeScript and Rust) that follow
  `next`/`prev` links without constructing URLs manually.

## Phase 4 – Background jobs and caching

### Step: Worker deployment

- [ ] Deploy Apalis worker pools in Kubernetes with queue partitioning, bounded
  retries, and dead-letter handling for `GenerateRoute` and `Enrichment` jobs.

### Step: Caching strategy

- [ ] Finalise the Redis caching adapter so requests share canonicalised keys,
  jittered TTLs, and metrics for hit/miss ratios before enabling caching in
  production.
- [ ] Implement cache invalidation hooks for schema or engine version upgrades,
  including namespace suffix rotation and eviction-safe rollouts.
- [ ] Add contract tests verifying canonicalisation rules (sorted themes,
  rounded coordinates, SHA-256 key format).

## Phase 5 – Map delivery and observability

### Step: Tile service rollout

- [ ] Deploy Martin with POI and route sources, read-only credentials, and
  ingress routing via `/tiles` or `tiles.wildside.app`.
- [ ] Add the `get_route_tile` PostGIS function, JWT validation path, and Grafana
  dashboards covering tile latency and error rates.

### Step: Observability extensions

- [ ] Install Prometheus exporters for API, workers, Redis, Postgres, and Martin,
  wiring alerts for latency spikes, queue backlogs, and cache eviction rates.
- [ ] Extend structured logging to include route IDs, user IDs, and trace IDs
  across HTTP, WebSocket, and worker boundaries with Loki dashboards.
- [ ] Instrument PostHog events (`RouteGenerated`, `UserSignup`, etc.) with
  consistent payload schemas and batching policies.

## Phase 6 – GitOps and environments

### Step: FluxCD resilience

- [ ] Harden FluxCD pipelines so manifests in `deploy/` reconcile cleanly across
  environments with image digest pinning and pull-request previews.

### Step: Operational runbooks

- [ ] Publish runbooks describing rolling upgrades, session-key rotation, and
  Martin or worker scaling procedures, referencing the observability dashboards.
- [ ] Automate preview-environment teardown and resource reclamation policies,
  including TTL enforcement and secret revocation.

## Phase 7 – Hexagonal consolidation

### Step: Domain isolation

- [ ] Create `backend/src/domain/mod.rs` by moving the existing `backend/src/models`
  module (errors, users, future route entities) and enforcing constructor
  validation plus immutable state.
- [ ] Introduce explicit port traits inside `backend/src/domain/ports.rs`
  (e.g. `RouteRepository`, `RouteCache`, `RouteQueue`, `RouteMetrics`,
  `UserRepository`) with strongly typed error enums instead of `anyhow::Result`.
- [ ] Replace direct DTO usage in `backend/src/api/*` with domain factories
  (e.g. `RouteRequest::try_from_login_payload`) so inbound adapters never
  construct domain structs manually.
- [ ] Convert shared error handling in `backend/src/models/error.rs` into domain
  error types that translate to HTTP responses via adapter-level mapping.

### Step: Adapter boundaries

- [ ] Move the current `backend/src/api` module into `backend/src/inbound/http`,
  keeping handlers thin (request parsing → domain service call → response
  mapping) and removing any persistence or session logic from the handler bodies.
- [ ] Rework the WebSocket entry point in `backend/src/ws` into an inbound adapter
  (`backend/src/inbound/ws`) that consumes domain events instead of building
  messages inline.
- [ ] Introduce `backend/src/outbound/persistence`, `backend/src/outbound/cache`,
  and `backend/src/outbound/queue` modules to encapsulate Diesel, Redis, and
  Apalis integrations once those backends are introduced, wiring them to the new
  port traits.
- [ ] Update all modules to depend on the domain ports rather than reaching into
  `backend/src/models` or framework-specific types, ensuring the dependency flow
  points inward.

### Step: Composition root and testing

- [ ] Extract server bootstrapping from `backend/src/main.rs` into
  `backend/src/app.rs`, constructing concrete adapters (HTTP, WS, persistence)
  and returning structs that the binary can run without exposing Actix types.
- [ ] Establish layered tests: in-domain unit tests with in-memory doubles for
  each port, adapter contract tests for `backend/src/outbound/*`, and
  end-to-end scenarios (HTTP + WebSocket) using the new composition root.
- [ ] Provide reusable test helpers under `backend/tests` to instantiate inbound
  adapters with mock services, ensuring every port has at least one
  collaborator-driven test.
- [ ] Update CI workflows to execute the unit, contract, and end-to-end suites
  separately, enforcing zero warnings and clippy cleanliness post-refactor.
