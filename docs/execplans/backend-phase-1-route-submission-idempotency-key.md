# Phase 1 route submission idempotency key

This Execution Plan (ExecPlan) is a living document. The sections `Progress`,
`Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective` must
be kept up to date as work proceeds.

There is no `PLANS.md` in this repository, so this ExecPlan is the sole
execution reference.

## Purpose / Big Picture

Implement idempotency for `POST /api/v1/routes` to ensure that duplicate or
retried route generation requests are handled gracefully. When a client submits
a request with an `Idempotency-Key` header:

- If the key is new, process the request and store the key-payload pair.
- If the key matches an existing request with identical payload, replay the
  original response without re-processing.
- If the key matches an existing request with a different payload, reject with
  `409 Conflict`.
- Keys expire after 24 hours (configurable via `ROUTES_IDEMPOTENCY_TTL_HOURS`).

Success is observable when:

- The `POST /api/v1/routes` endpoint accepts an `Idempotency-Key` header.
- Duplicate requests with matching keys and payloads replay the original
  response.
- Mismatched payloads for existing keys return `409 Conflict`.
- Idempotency records persist in PostgreSQL and survive server restarts.
- Records automatically expire after the configured time-to-live (TTL).
- Unit tests (`rstest`) cover domain logic for key validation and payload
  hashing.
- Behavioural tests (`rstest-bdd` v0.2.0) cover happy and unhappy paths against
  an embedded PostgreSQL instance.
- `docs/wildside-backend-architecture.md` records design decisions.
- `docs/backend-roadmap.md` marks the idempotency task as done.
- `make check-fmt`, `make lint`, and `make test` succeed.

## Progress

- [ ] Draft ExecPlan for route submission idempotency.
- [ ] Define domain types: `IdempotencyKey`, `IdempotencyRecord`,
  `IdempotencyLookupResult`.
- [ ] Define domain port: `IdempotencyStore` trait with error enum.
- [ ] Implement payload canonicalization and SHA-256 hashing.
- [ ] Create Diesel migration for `idempotency_keys` table.
- [ ] Update Diesel schema.rs with new table.
- [ ] Implement PostgreSQL adapter: `DieselIdempotencyStore`.
- [ ] Create domain service: `RouteSubmissionService` driving port.
- [ ] Implement HTTP handler: `POST /api/v1/routes` with idempotency middleware.
- [ ] Add `IdempotencyStore` to `HttpState`.
- [ ] Wire adapter in `main.rs`.
- [ ] Create feature file for BDD scenarios.
- [ ] Implement BDD step definitions.
- [ ] Create unit tests for domain types and hashing.
- [ ] Create contract tests for `IdempotencyStore` port.
- [ ] Update architecture documentation.
- [ ] Update roadmap to mark task complete.
- [ ] Run quality gates.

## Surprises & Discoveries

<!-- To be filled during implementation -->

## Decision Log

- Decision: Use SHA-256 hash of canonicalized JSON payload for conflict
  detection.
  Rationale: Canonicalized JSON (sorted keys, deterministic formatting) ensures
  semantically identical payloads produce the same hash regardless of
  whitespace or key ordering differences. SHA-256 provides collision resistance
  sufficient for this use case.
  Date/Author: 2025-12-23 / Claude Code.

- Decision: Store idempotency records in PostgreSQL rather than Redis.
  Rationale: The roadmap explicitly requires PostgreSQL persistence, so keys
  survive server restarts. Redis would be faster but would not meet the
  durability requirement without additional complexity. PostgreSQL with index
  on key provides adequate performance for expected load.
  Date/Author: 2025-12-23 / Claude Code.

- Decision: Make `Idempotency-Key` header optional; requests without it proceed
  normally without idempotency tracking.
  Rationale: Backwards compatibility with existing clients. Clients that want
  idempotency opt in by providing the header.
  Date/Author: 2025-12-23 / Claude Code.

- Decision: Use UUID v4 format for idempotency keys with server-side
  validation.
  Rationale: UUIDs provide sufficient entropy and are a common pattern for
  idempotency keys. Rejecting malformed keys early prevents storage of invalid
  data.
  Date/Author: 2025-12-23 / Claude Code.

- Decision: Create a driving port (`RouteSubmissionService`) rather than
  embedding idempotency logic directly in the HTTP handler.
  Rationale: Keeps the handler thin per hexagonal architecture. The service
  coordinates cache lookup, idempotency checking, and queue dispatch. This
  matches the pattern described in the architecture document for
  `POST /api/v1/routes`.
  Date/Author: 2025-12-23 / Claude Code.

- Decision: TTL enforcement via background cleanup job triggered by application
  startup or scheduled task, not per-query filtering.
  Rationale: Per-query `WHERE created_at > NOW() - interval` adds complexity
  to every lookup. A periodic cleanup (hourly or on startup) removes stale
  records while keeping lookups simple. Expired records that slip through are
  harmless—they just get cleaned up on the next sweep.
  Date/Author: 2025-12-23 / Claude Code.

## Outcomes & Retrospective

<!-- To be filled after implementation -->

## Context and Orientation

Key locations (repository-relative):

- `backend/src/domain/mod.rs`: domain module root.
- `backend/src/domain/ports/mod.rs`: port traits.
- `backend/src/outbound/persistence/mod.rs`: persistence adapters.
- `backend/src/outbound/persistence/schema.rs`: Diesel table definitions.
- `backend/src/inbound/http/mod.rs`: HTTP handler module.
- `backend/src/inbound/http/state.rs`: HTTP adapter state bundle.
- `backend/migrations/`: Diesel migrations.
- `backend/tests/`: integration and BDD tests.
- `docs/wildside-backend-architecture.md`: architecture documentation.
- `docs/backend-roadmap.md`: Phase 1 checklist entry to mark done.

Terminology (plain-language):

- *Idempotency key*: A client-provided unique identifier (UUID) sent via the
  `Idempotency-Key` HTTP header to enable safe request retries.
- *Payload hash*: SHA-256 hash of the canonicalized request body, used to
  detect conflicting payloads for the same key.
- *Replay*: Returning a previously stored response for a matching idempotency
  key without re-processing the request.
- *Conflict*: When a client reuses an idempotency key with a different payload
  than the original request.
- *TTL*: Time-to-live; the duration (default 24 hours) after which idempotency
  records expire and are eligible for cleanup.

## Plan of Work

### 1. Domain types (backend/src/domain/)

Create `idempotency.rs`:

- `IdempotencyKey`: Newtype around UUID with validation.
- `IdempotencyRecord`: Stores key, payload hash, response snapshot, timestamps.
- `IdempotencyLookupResult`: Enum with variants `NotFound`, `MatchingPayload`,
  `ConflictingPayload`.
- `PayloadHash`: Newtype for SHA-256 hash bytes.

Add unit tests for:

- Key validation (valid UUID format).
- Key rejection for malformed input.

### 2. Payload canonicalization (backend/src/domain/idempotency.rs)

Implement `canonicalize_and_hash`:

- Accept a `serde_json::Value`.
- Sort object keys recursively.
- Serialize to compact JSON (no whitespace).
- Hash with SHA-256.
- Return `PayloadHash`.

Add unit tests for:

- Deterministic output regardless of input key order.
- Identical output for semantically equivalent payloads.
- Different output for different payloads.

### 3. Domain port (backend/src/domain/ports/)

Create `idempotency_store.rs`:

```rust
#[async_trait]
pub trait IdempotencyStore: Send + Sync {
    async fn lookup(
        &self,
        key: &IdempotencyKey,
        payload_hash: &PayloadHash,
    ) -> Result<IdempotencyLookupResult, IdempotencyStoreError>;

    async fn store(&self, record: &IdempotencyRecord) -> Result<(), IdempotencyStoreError>;

    async fn cleanup_expired(&self, ttl: Duration) -> Result<u64, IdempotencyStoreError>;
}
```

The `lookup` method requires both the key and payload hash because the store
must compare the incoming hash against any stored record to determine whether
the request is a replay (matching hash) or a conflict (different hash).

Define `IdempotencyStoreError` enum using `define_port_error!` macro:

- `Connection`: database connection failure.
- `Query`: query execution failure.
- `Serialization`: response serialization failure.

### 4. Driving port (backend/src/domain/ports/)

Create `route_submission_service.rs`:

```rust
#[async_trait]
pub trait RouteSubmissionService: Send + Sync {
    async fn submit(&self, request: RouteSubmissionRequest) -> Result<RouteSubmissionResponse, Error>;
}
```

Where `RouteSubmissionRequest` contains:

- `idempotency_key: Option<IdempotencyKey>`
- `user_id: UserId`
- `payload: RouteRequestPayload`

And `RouteSubmissionResponse` contains:

- `request_id: Uuid`
- `status: RouteSubmissionStatus` (enum: `Accepted`, `Replayed`)

### 5. Diesel migration

Create `backend/migrations/<timestamp>_create_idempotency_keys/`:

`up.sql`:

```sql
CREATE TABLE idempotency_keys (
    key UUID PRIMARY KEY,
    payload_hash BYTEA NOT NULL,
    response_snapshot JSONB NOT NULL,
    user_id UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_idempotency_keys_created_at ON idempotency_keys (created_at);
```

`down.sql`:

```sql
DROP TABLE IF EXISTS idempotency_keys;
```

### 6. Update Diesel schema

Add to `backend/src/outbound/persistence/schema.rs`:

```rust
diesel::table! {
    idempotency_keys (key) {
        key -> Uuid,
        payload_hash -> Bytea,
        response_snapshot -> Jsonb,
        user_id -> Uuid,
        created_at -> Timestamptz,
    }
}
```

### 7. PostgreSQL adapter (backend/src/outbound/persistence/)

Create `diesel_idempotency_store.rs`:

- Implement `IdempotencyStore` trait using `diesel-async`.
- Define internal models: `IdempotencyKeyRow`, `NewIdempotencyKeyRow`.
- Map Diesel errors to `IdempotencyStoreError`.
- Use `bb8` connection pool (shared with other adapters).

### 8. Domain service implementation

Create `backend/src/domain/route_submission.rs`:

- Implement a concrete `RouteSubmissionServiceImpl` that:
  - Checks idempotency store if key provided.
  - On match: return replayed response.
  - On conflict: return error.
  - On miss or no key: enqueue job, store record, return accepted.
- Inject `IdempotencyStore` and `RouteQueue` ports.

### 9. HTTP handler (backend/src/inbound/http/)

Create `routes.rs`:

- `POST /api/v1/routes` handler.
- Extract `Idempotency-Key` header (optional).
- Validate key format if present.
- Parse and validate JSON body into `RouteRequestPayload`.
- Delegate to `RouteSubmissionService`.
- Return appropriate HTTP status:
  - `202 Accepted` with `Location` header for new/replayed requests.
  - `409 Conflict` for mismatched payloads.
  - `400 Bad Request` for invalid key format or payload.

### 10. HTTP state update

Update `backend/src/inbound/http/state.rs`:

- Add `idempotency: Arc<dyn IdempotencyStore>` field.
- Add `route_submission: Arc<dyn RouteSubmissionService>` field.
- Update `HttpState::new` constructor.

### 11. Wire in main.rs

- Create `DieselIdempotencyStore` instance.
- Create `RouteSubmissionServiceImpl` instance.
- Add to `HttpState`.
- Register `/api/v1/routes` endpoint.
- Add periodic cleanup task (on startup or via Apalis scheduler stub).

### 12. BDD tests

Create `backend/tests/features/route_submission_idempotency.feature`:

```gherkin
Feature: Route submission idempotency

  Scenario: First request with idempotency key is accepted
    Given a postgres-backed idempotency store
    And a valid route submission request
    When the request is submitted with a fresh idempotency key
    Then the response status is 202 Accepted
    And a request_id is returned

  Scenario: Duplicate request with matching payload replays response
    Given a postgres-backed idempotency store
    And a stored idempotency record for a previous request
    When the same request is submitted with the same idempotency key
    Then the response status is 202 Accepted
    And the same request_id is returned

  Scenario: Duplicate key with different payload is rejected
    Given a postgres-backed idempotency store
    And a stored idempotency record for a previous request
    When a different payload is submitted with the same idempotency key
    Then the response status is 409 Conflict
    And an error message indicates conflicting payload

  Scenario: Request without idempotency key proceeds normally
    Given a postgres-backed idempotency store
    And a valid route submission request
    When the request is submitted without an idempotency key
    Then the response status is 202 Accepted
    And a request_id is returned

  Scenario: Invalid idempotency key format is rejected
    Given a postgres-backed idempotency store
    And a valid route submission request
    When the request is submitted with an invalid idempotency key
    Then the response status is 400 Bad Request
    And an error message indicates invalid key format
```

Create `backend/tests/route_submission_idempotency_bdd.rs`:

- Follow patterns from `ports_behaviour.rs`.
- Use `pg_embedded_setup_unpriv::TestCluster` fixture.
- Implement step definitions for all scenarios.

### 13. Contract tests for IdempotencyStore

Create `backend/tests/idempotency_store_contract.rs`:

- Test `lookup` returns `NotFound` for unknown keys.
- Test `store` persists record.
- Test `lookup` returns `MatchingPayload` when hash matches.
- Test `lookup` returns `ConflictingPayload` when hash differs.
- Test `cleanup_expired` removes old records.

### 14. Unit tests

Add to domain module tests:

- `IdempotencyKey::new` accepts valid UUIDs.
- `IdempotencyKey::new` rejects invalid strings.
- `canonicalize_and_hash` determinism tests.
- `PayloadHash` equality semantics.

### 15. Documentation updates

Update `docs/wildside-backend-architecture.md`:

- Add section on idempotency handling in REST API Specification.
- Document the `Idempotency-Key` header behaviour.
- Add design decision for PostgreSQL storage.

Update `docs/backend-roadmap.md`:

- Mark the route submission idempotency task as done.

### 16. Quality gates

Run and verify:

- `make check-fmt`
- `make lint`
- `make test`
- `make markdownlint`

## Concrete Steps

Run these commands from the repository root. Use a 300-second timeout by
default and capture output with `tee` so logs are preserved.

1. After code changes:

   ```bash
   set -o pipefail
   timeout 300 make fmt 2>&1 | tee /tmp/wildside-fmt.log
   ```

2. Check formatting:

   ```bash
   set -o pipefail
   timeout 300 make check-fmt 2>&1 | tee /tmp/wildside-check-fmt.log
   ```

3. Lint:

   ```bash
   set -o pipefail
   timeout 300 make lint 2>&1 | tee /tmp/wildside-lint.log
   ```

4. Test (may take longer than 300s):

   ```bash
   set -o pipefail
   timeout 600 make test 2>&1 | tee /tmp/wildside-test.log
   ```

5. Markdown lint (if documentation updated):

   ```bash
   set -o pipefail
   timeout 300 make markdownlint 2>&1 | tee /tmp/wildside-markdownlint.log
   ```

If running tests locally without elevated permissions for the Postgres worker,
use the helper described in `docs/pg-embed-setup-unpriv-users-guide.md`:

```bash
set -o pipefail
PG_WORKER_PATH=/tmp/pg_worker timeout 600 make test 2>&1 \
    | tee /tmp/wildside-test.log
```

## Validation and Acceptance

Acceptance criteria:

1. Idempotency header support:
   - `POST /api/v1/routes` accepts `Idempotency-Key` header.
   - Valid UUID format is enforced.
   - Invalid keys return `400 Bad Request`.

2. Idempotency behaviour:
   - First request with key returns `202 Accepted`.
   - Duplicate request with same payload replays response.
   - Duplicate request with different payload returns `409 Conflict`.
   - Request without key proceeds without idempotency tracking.

3. Persistence:
   - Records stored in PostgreSQL `idempotency_keys` table.
   - Records survive server restart.
   - Expired records cleaned up after TTL.

4. Testing:
   - Unit tests cover key validation and payload hashing.
   - BDD tests cover all scenarios against embedded PostgreSQL.
   - Contract tests validate `IdempotencyStore` port semantics.

5. Documentation and quality:
   - Architecture documentation updated.
   - Roadmap task marked complete.
   - All quality gates pass.

## Idempotence and Recovery

- Domain types are pure and testable.
- Diesel migrations are tracked in the `__diesel_schema_migrations` table,
  preventing re-execution of already-applied migrations.
- Store operations use UPSERT semantics where appropriate.
- If a command fails, fix the issue and re-run only the failed command.

## Artifacts and Notes

Keep log files created by the `tee` commands until the work is complete:

- `/tmp/wildside-check-fmt.log`
- `/tmp/wildside-lint.log`
- `/tmp/wildside-test.log`
- `/tmp/wildside-markdownlint.log`

## Interfaces and Dependencies

New domain types in `backend/src/domain/idempotency.rs`:

```rust
/// Validated idempotency key (UUID v4).
pub struct IdempotencyKey(Uuid);

/// SHA-256 hash of canonicalized request payload.
pub struct PayloadHash([u8; 32]);

/// Stored idempotency record.
pub struct IdempotencyRecord {
    pub key: IdempotencyKey,
    pub payload_hash: PayloadHash,
    pub response_snapshot: serde_json::Value,
    pub user_id: UserId,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Result of idempotency lookup.
pub enum IdempotencyLookupResult {
    NotFound,
    MatchingPayload(IdempotencyRecord),
    ConflictingPayload(IdempotencyRecord),
}
```

New port trait in `backend/src/domain/ports/idempotency_store.rs`:

```rust
#[async_trait]
pub trait IdempotencyStore: Send + Sync {
    async fn lookup(
        &self,
        key: &IdempotencyKey,
        payload_hash: &PayloadHash,
    ) -> Result<IdempotencyLookupResult, IdempotencyStoreError>;

    async fn store(
        &self,
        record: &IdempotencyRecord,
    ) -> Result<(), IdempotencyStoreError>;

    async fn cleanup_expired(
        &self,
        ttl: std::time::Duration,
    ) -> Result<u64, IdempotencyStoreError>;
}
```

Updated `HttpState` in `backend/src/inbound/http/state.rs`:

```rust
pub struct HttpState {
    pub login: Arc<dyn LoginService>,
    pub users: Arc<dyn UsersQuery>,
    pub profile: Arc<dyn UserProfileQuery>,
    pub interests: Arc<dyn UserInterestsCommand>,
    pub route_submission: Arc<dyn RouteSubmissionService>,  // NEW
}
```
