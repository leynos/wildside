# Wildside backend roadmap

This roadmap captures the outstanding delivery work for the Wildside backend.
It follows the phase -> step -> task hierarchy defined in
`docs/documentation-style-guide.md`. Progress is tracked with checkboxes; tasks
may only be marked complete when their acceptance criteria are met and the
related automated checks pass.

The backend must uphold the hexagonal modular monolith described in
`docs/wildside-backend-architecture.md` from the outset of every change. Hex
architecture is not a future refactor: every deliverable must lean on the
domain, port, and adapter seams established in section 1. No downstream task
can be considered done if it violates these constraints.

## 1. Hexagonal foundations

These steps gate all other phases. Complete them before tackling feature
delivery, so future work remains inside the hexagonal boundaries.

### 1.1. Domain and ports baseline

- [x] 1.1.1. Create `backend/src/domain/mod.rs` by moving the existing
  `backend/src/models` module (errors, users, and future route entities) and
  enforcing constructor validation plus immutable state.
- [x] 1.1.2. Introduce explicit port traits inside `backend/src/domain/ports.rs`
  (for example, `RouteRepository`, `RouteCache`, `RouteQueue`, `RouteMetrics`,
  and `UserRepository`) with strongly typed error enums instead of
  `anyhow::Result`.
- [x] 1.1.3. Replace direct data transfer object usage in `backend/src/api/*`
  with domain factories (for example, `RouteRequest::try_from_login_payload`)
  so inbound adapters never construct domain structs manually.
- [x] 1.1.4. Convert shared error handling in `backend/src/models/error.rs`
  into domain error types that translate to HTTP responses via adapter-level
  mapping.

### 1.2. Adapter boundaries

- [x] 1.2.1. Move the current `backend/src/api` module into
  `backend/src/inbound/http`, keeping handlers thin (request parsing -> domain
  service call -> response mapping) and ensuring handler bodies only coordinate
  domain calls and high-level session helpers (no direct framework-specific
  session manipulation).
- [x] 1.2.2. Rework the WebSocket entry point in `backend/src/ws` into an
  inbound adapter (`backend/src/inbound/ws`) that consumes domain events
  instead of building messages inline.
- [x] 1.2.3. Introduce `backend/src/outbound/persistence`,
  `backend/src/outbound/cache`, and `backend/src/outbound/queue` modules to
  encapsulate Diesel, Redis, and Apalis integrations once those backends are
  introduced, wiring them to the new port traits.
- [x] 1.2.4. Update all modules to depend on the domain ports rather than
  reaching into `backend/src/models` or framework-specific types, ensuring the
  dependency flow points inward.

### 1.3. Architecture guardrails

- [x] 1.3.1. Extend `docs/wildside-backend-architecture.md` with inbound and
  outbound module diagrams, port usage examples, and a checklist for
  introducing new adapters, so the boundaries stay visible.
- [x] 1.3.2. Add an architectural lint (for example, dependency allowlists
  enforced via `cargo deny` or a custom build script) that fails when inbound
  adapters import outbound modules or infrastructure crates directly, and wire
  it into `make lint`.
- [x] 1.3.3. Provide integration tests that exercise HTTP and WebSocket
  handlers against mocked ports to ensure adapters remain side effect free and
  domain logic stays framework-agnostic.

## 2. Core access and sessions

All delivery in this phase must consume domain services via the inbound HTTP
adapter; direct stateful logic belongs behind the ports established above.

### 2.1. Session lifecycle hardening

- [x] 2.1.1. Implement `POST /api/v1/login` with signed-cookie sessions, two-
  hour time-to-live (TTL), and production-grade key management.
- [x] 2.1.2. Wire `/api/v1/users/me` and `/api/v1/users/me/interests` to require
  the session middleware, returning `401` with trace identifiers when
  unauthenticated.
- [x] 2.1.3. Enforce configuration toggles for `SESSION_SAMESITE`,
  `SESSION_COOKIE_SECURE`, and `SESSION_ALLOW_EPHEMERAL`, failing fast in
  release builds when secrets are missing or keys are too short.
- [x] 2.1.4. Document and script the rotation procedure for session signing
  keys, including Kubernetes secret rollout and dual validation during deploys.

### 2.2. Route submission idempotency

- [x] 2.2.1. Persist `Idempotency-Key` headers for `POST /api/v1/routes` in
  PostgreSQL, rejecting conflicting payloads with `409` and replaying
  successful responses within 24 hours.
- [x] 2.2.2. Introduce a shared `IdempotencyRepository` with configurable
  time-to-live (TTL) and reuse it for outbox-backed mutations (notes, progress,
  preferences, and offline bundles).
- [x] 2.2.3. Capture idempotency audit metrics (hits, misses, and conflicts)
  and expose them via Prometheus with labels for user scope and key age buckets.

### 2.3. Progressive web app preferences and annotations

- [x] 2.3.1. Add domain types for `UserPreferences`, `RouteNote`, and
  `RouteProgress`, plus ports `UserPreferencesRepository`,
  `RouteAnnotationRepository`, and driving commands that enforce revision
  checks.
- [x] 2.3.2. Implement `GET/PUT /api/v1/users/me/preferences`,
  `GET /api/v1/routes/{route_id}/annotations`,
  `POST /api/v1/routes/{route_id}/notes`, and
  `PUT /api/v1/routes/{route_id}/progress` with idempotency headers and
  consistent error envelopes.
- [x] 2.3.3. Add contract tests covering optimistic concurrency, idempotency
  conflicts, and deterministic responses for retried requests.

### 2.4. Example data seeding

- [x] 2.4.1. Draft the design and ExecPlan for example data seeding and the
  `example-data` crate (`docs/execplans/backend-sample-data-design.md`).
- [x] 2.4.2. Implement the `example-data` crate using the `fake` crate with
  deterministic generation, JSON seed registry parsing, and display-name
  validation.
- [x] 2.4.3. Add the `example_data_runs` migration plus a repository helper to
  guard seeding once per seed name.
- [x] 2.4.4. Deliver a seed registry CLI that uses `base-d` (`eff_long`) to
  generate memorable seed names and updates the JSON registry safely.
- [x] 2.4.5. Wire startup seeding behind the `example-data` feature flag and
  `ortho-config` settings, logging when seeding is skipped or applied.
- [x] 2.4.6. Add integration tests for once-only seeding and update backend
  documentation to describe the demo data flow.

## 3. Data platform foundation

Ensure schema and ingestion work expose their operations through domain ports,
so persistence details stay confined to outbound adapters.

### 3.1. Schema baseline

- [x] 3.1.1. Deliver Diesel migrations that materialize the schema in
  `docs/wildside-backend-architecture.md`, including catalogue, descriptor, and
  user state tables plus GiST/GIN indices and unique constraints for composite
  keys.
- [x] 3.1.2. Generate entity-relationship (ER) diagram snapshots from
  migrations and store them alongside documentation for traceability.

### 3.2. Catalogue and descriptor read models

- [x] 3.2.1. Define catalogue and descriptor domain types (`RouteSummary`,
  `RouteCategory`, `Theme`, `RouteCollection`, `TrendingRouteHighlight`,
  `CommunityPick`, `Tag`, `Badge`, `SafetyToggle`, and `SafetyPreset`) with
  localization maps and semantic icon identifiers.
- [x] 3.2.2. Add `CatalogueRepository` and `DescriptorRepository` ports plus
  persistence adapters with contract tests for localization payloads.
- [x] 3.2.3. Implement `GET /api/v1/catalogue/explore` and
  `GET /api/v1/catalogue/descriptors` endpoints backed by the read models, with
  cache headers and snapshot `generated_at` metadata.

### 3.3. Offline bundles and walk completion

- [x] 3.3.1. Add `OfflineBundle` and `WalkSession` domain types plus
  repositories for manifests and completion summaries.
- [x] 3.3.2. Deliver migrations for `offline_bundles` and `walk_sessions` with
  audit timestamps and bounds/zoom metadata.
- [x] 3.3.3. Implement `GET/POST/DELETE /api/v1/offline/bundles` and
  `POST /api/v1/walk-sessions` endpoints, returning stable identifiers and
  revision updates where applicable.

### 3.4. Data ingestion and enrichment

- [x] 3.4.1. Ship the Rust-based `ingest-osm` command-line interface (CLI) by
  integrating [`wildside-engine`](https://github.com/leynos/wildside-engine)
  ingestion capabilities through `wildside-data` (the library underpinning
  `wildside-cli ingest`) and documenting:
  - backend-owned behaviour:
    - launch geofence filtering.
    - provenance persistence (source URL, input digest, timestamp, and
      bounding box).
    - deterministic reruns keyed by geofence and input digest.
- [x] 3.4.2. Add Overpass enrichment workers with semaphore-governed quotas,
  circuit breaking, and metrics wired to the enrichment job counters.
- [x] 3.4.3. Configure enrichment provenance persistence (source URL,
  timestamp, and bounding box) and expose it via admin reporting endpoints.

### 3.5. User state port persistence parity

- [x] 3.5.1. Audit current schema coverage for login, users, profile, and
  interests persistence, then document whether new migrations are required for
  profile and interests storage, revision tracking, and update conflict
  handling.
- [ ] 3.5.2. Replace fixture-backed `LoginService` and `UsersQuery` wiring in
  server state construction with explicit DB-backed concrete types, either by
  extending `DieselUserRepository` to satisfy those ports directly or by
  introducing adapter wrappers around it, while preserving current session and
  error-envelope behaviour.
- [ ] 3.5.3. Replace fixture-backed `UserProfileQuery` and
  `UserInterestsCommand` wiring with explicit DB-backed concrete types, and
  document whether this uses repository extensions (for example
  `DieselUserRepository`) or dedicated adapters (for example
  `DieselProfileRepository` and `DieselInterestsRepository`).
- [ ] 3.5.4. Define and implement the revision-safe interests update strategy
  (for example optimistic concurrency via expected revision checks), including
  the persistence contract and error mapping for stale-write conflicts.
- [ ] 3.5.5. Update `backend/src/server/state_builders.rs` so
  `login/users/profile/interests` select DB-backed implementations when
  `config.db_pool` is present and retain fixture fallbacks when it is absent.
- [ ] 3.5.6. Add behavioural and repository-level tests covering the new
  adapter wiring paths, including DB-present and fixture-fallback startup
  modes, plus revision-conflict cases for interests updates.

## 4. Pagination infrastructure

Pagination relies on domain repositories exposing ordered queries via ports;
see `docs/keyset-pagination-design.md` for the detailed crate design.

### 4.1. Pagination crate foundation

- [ ] 4.1.1. Implement `backend/crates/pagination` providing opaque cursor
  encoding, `PageParams`, and `Paginated<T>` envelopes with navigation links,
  backed by unit tests for cursor round-tripping.
- [ ] 4.1.2. Add support for direction-aware cursors (`Next` and `Prev`) with
  serde-based encoding and property tests ensuring decode-encode stability.
- [ ] 4.1.3. Publish crate-level documentation outlining ordering
  requirements, default and maximum limits (20 and 100), and error mapping
  guidelines.

### 4.2. Endpoint adoption

- [ ] 4.2.1. Replace offset pagination in `GET /api/users` with the new crate,
  including Diesel filters that respect `(created_at, id)` ordering and bb8
  connection pooling.
- [ ] 4.2.2. Update the repository layer to surface pagination-aware errors
  (for example, invalid cursor format and unsupported direction) with HTTP 400
  responses.
- [ ] 4.2.3. Ensure pagination telemetry records page size, cursor direction,
  and page traversal counts for analytics.
- [ ] 4.2.4. Add cursor-based pagination adoption for
  `GET /api/v1/admin/enrichment/provenance` while preserving current
  deterministic ordering on `(imported_at, id)` and repository-port boundaries.
- [ ] 4.2.5. Implement dual query compatibility for admin enrichment reporting:
  accept legacy `before` and new opaque `cursor` during migration, reject
  requests that provide both, and map invalid cursor or direction inputs to
  HTTP `400`.
- [ ] 4.2.6. Introduce transitional response compatibility for admin enrichment
  reporting by adding cursor-navigation fields (`nextCursor` and hypermedia
  links) while retaining `nextBefore` until client migration is complete.
- [ ] 4.2.7. Extend repository and endpoint tests to prove lossless traversal
  across `(imported_at, id)` tie boundaries for cursor mode, plus regression
  coverage that legacy `before` behaviour remains stable during migration.
- [ ] 4.2.8. Execute and document the deprecation plan for legacy `before` /
  `nextBefore` support, including removal criteria and release sequencing once
  consumers have migrated to opaque cursors.

### 4.3. Documentation and quality gates

- [ ] 4.3.1. Update the OpenAPI schema, AsyncAPI artefacts, and developer
  guides to document the new `cursor` and `limit` query parameters and response
  envelope.
- [ ] 4.3.2. Add integration tests exercising forward and backward
  pagination, plus contract tests guaranteeing link generation and page-size
  guardrails.
- [ ] 4.3.3. Provide sample client implementations (TypeScript and Rust) that
  follow `next` and `prev` links without constructing URLs manually.

## 5. Background jobs and caching

Background workers and caches must interact with the domain exclusively via the
queue, cache, and repository ports defined in section 1.

### 5.1. Cache adapter (Redis)

- [ ] 5.1.1. Implement `RouteCache` using Redis with `bb8-redis` for connection
  pooling, replacing the current stub adapter.
- [ ] 5.1.2. Add serialization with `serde_json` for cached plan payloads.
- [ ] 5.1.3. Implement time-to-live (TTL) with jitter (24-hour window, +/- 10%)
  to prevent thundering herd on cache expiry.
- [ ] 5.1.4. Add contract tests for cache key canonicalization (sorted themes,
  rounded coordinates, Secure Hash Algorithm 256-bit (SHA-256) key format).

### 5.2. Queue adapter (Apalis)

- [ ] 5.2.1. Implement `RouteQueue` using Apalis with PostgreSQL backend,
  replacing the current stub adapter.
- [ ] 5.2.2. Define job structs for `GenerateRouteJob` and `EnrichmentJob`.
- [ ] 5.2.3. Implement retry policies with exponential backoff and dead-letter
  handling.
- [ ] 5.2.4. Propagate trace IDs through job metadata for observability.

### 5.3. Worker deployment

- [ ] 5.3.1. Deploy Apalis worker pools in Kubernetes with queue partitioning,
  bounded retries, and dead-letter handling for `GenerateRoute` and
  `Enrichment` jobs.

### 5.4. Caching strategy

- [ ] 5.4.1. Finalize the Redis caching adapter, so requests share
  canonicalized keys, jittered TTLs, and metrics for hit-and-miss ratios before
  enabling caching in production.
- [ ] 5.4.2. Implement cache invalidation hooks for schema or engine version
  upgrades, including namespace suffix rotation and eviction-safe rollouts.
- [ ] 5.4.3. Add contract tests verifying canonicalization rules (sorted
  themes, rounded coordinates, SHA-256 key format).

## 6. Map delivery and observability

Tile serving and telemetry must remain adapter concerns; domain services
publish events and metrics through their ports only.

### 6.1. Tile service rollout

- [ ] 6.1.1. Deploy Martin with point of interest (POI) and route sources,
  read-only credentials, and ingress routing via `/tiles` or
  `tiles.wildside.app`.
- [ ] 6.1.2. Add the `get_route_tile` PostGIS function, JSON Web Token (JWT)
  validation path, and Grafana dashboards covering tile latency and error rates.

### 6.2. Observability extensions

- [ ] 6.2.1. Install Prometheus exporters for API, workers, Redis, Postgres,
  and Martin, wiring alerts for latency spikes, queue backlogs, and cache
  eviction rates.
- [ ] 6.2.2. Extend structured logging to include route identifiers, user
  identifiers, and trace IDs across HTTP, WebSocket, and worker boundaries with
  Loki dashboards.
- [ ] 6.2.3. Instrument PostHog events (`RouteGenerated`, `UserSignup`, and
  similar) with consistent payload schemas and batching policies.

## 7. Deployment coordination

Deployment automation, preview environment workflows, and operational runbooks
are maintained in the Nile Valley repository. Coordinate roadmap changes with
that repository when deployment or infrastructure behaviour needs updates.
