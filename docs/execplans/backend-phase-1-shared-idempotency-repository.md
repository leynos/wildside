# Phase 1 shared idempotency repository

This Execution Plan (ExecPlan) is a living document. The sections `Progress`,
`Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective` must
be kept up to date as work proceeds.

There is no `PLANS.md` in this repository, so this ExecPlan is the sole
execution reference.

## Purpose / Big Picture

Introduce a shared `IdempotencyRepository` port with configurable time-to-live
(TTL) that can be reused across all outbox-backed mutations:

- Route submission (existing)
- Route notes
- Route progress
- User preferences
- Offline bundles

The existing `IdempotencyStore` port was designed specifically for route
submission. This work generalizes it into a repository pattern that supports
multiple mutation types with a configurable TTL, enabling consistent
idempotency semantics across the backend.

Success is observable when:

- The `IdempotencyStore` port is renamed to `IdempotencyRepository` and
  generalized for multiple mutation types.
- A `MutationType` enum distinguishes between idempotency scopes (routes, notes,
  progress, preferences, bundles).
- TTL is configurable via `IDEMPOTENCY_TTL_HOURS` environment variable with a
  default of 24 hours for backward compatibility.
- The Diesel adapter (`DieselIdempotencyStore`) is updated to implement the
  renamed port.
- The `RouteSubmissionServiceImpl` uses the updated port without behaviour
  changes.
- Database schema includes a `mutation_type` column for future filtering.
- Unit tests (`rstest`) cover the new `MutationType` enum and TTL configuration.
- Behavioural tests (`rstest-bdd` v0.3.1, Behaviour-Driven Development (BDD)
  style) cover happy and unhappy paths against an embedded PostgreSQL instance.
- `docs/wildside-backend-architecture.md` records design decisions.
- `docs/backend-roadmap.md` marks the task as done.
- `make check-fmt`, `make lint`, and `make test` succeed.

## Progress

- [ ] Draft ExecPlan for shared idempotency repository.
- [ ] Define `MutationType` enum in domain.
- [ ] Define `IdempotencyConfig` for configurable TTL.
- [ ] Rename `IdempotencyStore` to `IdempotencyRepository`.
- [ ] Update `IdempotencyRecord` to include `mutation_type`.
- [ ] Create Diesel migration for `mutation_type` column.
- [ ] Update `DieselIdempotencyStore` to `DieselIdempotencyRepository`.
- [ ] Update `RouteSubmissionServiceImpl` to use renamed port.
- [ ] Update `HttpState` to use renamed types.
- [ ] Update fixture implementation (`FixtureIdempotencyStore`).
- [ ] Update mock implementation.
- [ ] Create unit tests for `MutationType` and `IdempotencyConfig`.
- [ ] Create contract tests for `IdempotencyRepository` port.
- [ ] Create BDD feature file and step definitions.
- [ ] Update architecture documentation.
- [ ] Update roadmap to mark task complete.
- [ ] Run quality gates.

## Surprises & Discoveries

<!-- To be filled during implementation -->

## Decision Log

- Decision: Rename `IdempotencyStore` to `IdempotencyRepository` for consistency
  with other repository ports.
  Rationale: The architecture document already refers to `IdempotencyRepository`
  in the driven ports section. Aligning naming improves developer navigation and
  matches the established pattern (`UserRepository`, `RouteRepository`, etc.).
  Date/Author: 2025-12-28 / Claude Code.

- Decision: Add `MutationType` enum to scope idempotency records by mutation
  kind.
  Rationale: Without a type discriminator, keys could collide if different
  mutations happen to use the same UUID. Scoping by type ensures isolation
  between route submissions, note upserts, preference updates, and other
  outbox-backed operations.
  Date/Author: 2025-12-28 / Claude Code.

- Decision: Store `mutation_type` as a TEXT column with CHECK constraint rather
  than a separate lookup table.
  Rationale: The set of mutation types is small and stable (routes, notes,
  progress, preferences, bundles). A CHECK constraint provides integrity without
  join overhead. Migration path is simpler and the column is self-documenting.
  Date/Author: 2025-12-28 / Claude Code.

- Decision: Use `IDEMPOTENCY_TTL_HOURS` environment variable for TTL
  configuration with 24-hour default.
  Rationale: The existing route submission idempotency uses a 24-hour TTL via
  `ROUTES_IDEMPOTENCY_TTL_HOURS`. A shared config simplifies operations while
  preserving the existing behaviour. Future work can add per-type TTL overrides
  if needed.
  Date/Author: 2025-12-28 / Claude Code.

- Decision: Make `mutation_type` column NOT NULL with a default of `'routes'`
  for backward compatibility.
  Rationale: Existing records from route submission should continue to work
  without migration issues. The default ensures `mutation_type` is always
  populated even for records created before this migration.
  Date/Author: 2025-12-28 / Claude Code.

- Decision: Include `mutation_type` in the composite primary key alongside `key`
  and `user_id`.
  Rationale: This allows the same UUID to be used as an idempotency key across
  different mutation types without collision. The existing composite key
  `(key, user_id)` becomes `(key, user_id, mutation_type)`.
  Date/Author: 2025-12-28 / Claude Code.

## Outcomes & Retrospective

<!-- To be filled after implementation -->

## Context and Orientation

Key locations (repository-relative):

- `backend/src/domain/idempotency/mod.rs`: Domain types (`IdempotencyKey`,
  `IdempotencyRecord`, `PayloadHash`).
- `backend/src/domain/ports/idempotency_repository.rs`: Port trait.
- `backend/src/domain/ports/mod.rs`: Port module root.
- `backend/src/domain/route_submission/mod.rs`: `RouteSubmissionServiceImpl`.
- `backend/src/outbound/persistence/diesel_idempotency_repository.rs`: Diesel
  adapter.
- `backend/src/outbound/persistence/models.rs`: Diesel models.
- `backend/src/outbound/persistence/schema.rs`: Diesel table definitions.
- `backend/src/inbound/http/state.rs`: HTTP adapter state bundle.
- `backend/src/server/mod.rs`: Server wiring.
- `backend/migrations/`: Diesel migrations.
- `backend/tests/`: Integration and BDD tests.
- `docs/wildside-backend-architecture.md`: Architecture documentation.
- `docs/backend-roadmap.md`: Phase 1 checklist entry to mark done.

Terminology (plain-language):

- *Idempotency repository*: A port trait defining storage and retrieval of
  idempotency records, now generalised for multiple mutation types.
- *Mutation type*: An enum discriminating between different outbox-backed
  operations (routes, notes, progress, preferences, bundles).
- *TTL*: Time-to-live; the duration after which idempotency records expire and
  are eligible for cleanup. Configurable via environment variable.
- *Outbox-backed mutation*: A write operation that uses the idempotency pattern
  to ensure exactly-once semantics for client retries.

## Plan of Work

### 1. Define `MutationType` enum (backend/src/domain/idempotency/mod.rs)

Add a new enum representing the scope of idempotency:

```rust
/// The type of mutation protected by idempotency.
///
/// Each variant corresponds to an outbox-backed operation that supports
/// idempotent retries. The discriminator ensures keys are isolated per
/// mutation kind, preventing collisions when different operations use
/// the same UUID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationType {
    /// Route submission (`POST /api/v1/routes`).
    Routes,
    /// Route note upsert (`POST /api/v1/routes/{route_id}/notes`).
    Notes,
    /// Route progress update (`PUT /api/v1/routes/{route_id}/progress`).
    Progress,
    /// User preferences update (`PUT /api/v1/users/me/preferences`).
    Preferences,
    /// Offline bundle operations (`POST/DELETE /api/v1/offline/bundles`).
    Bundles,
}

impl MutationType {
    /// Returns the database string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Routes => "routes",
            Self::Notes => "notes",
            Self::Progress => "progress",
            Self::Preferences => "preferences",
            Self::Bundles => "bundles",
        }
    }
}

impl std::fmt::Display for MutationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
```

Add unit tests for serialization round-tripping and `as_str` correctness.

### 2. Define `IdempotencyConfig` (backend/src/domain/idempotency/mod.rs)

Add a configuration struct for TTL:

```rust
/// Configuration for idempotency behaviour.
#[derive(Debug, Clone)]
pub struct IdempotencyConfig {
    /// Time-to-live for idempotency records.
    pub ttl: Duration,
}

impl IdempotencyConfig {
    /// Load configuration from environment.
    ///
    /// Reads `IDEMPOTENCY_TTL_HOURS` (default: 24).
    pub fn from_env() -> Self {
        let hours = std::env::var("IDEMPOTENCY_TTL_HOURS")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(24);
        Self {
            ttl: Duration::from_secs(hours * 3600),
        }
    }

    /// Create with explicit TTL (for testing).
    pub fn with_ttl(ttl: Duration) -> Self {
        Self { ttl }
    }
}

impl Default for IdempotencyConfig {
    fn default() -> Self {
        Self {
            ttl: Duration::from_secs(24 * 3600),
        }
    }
}
```

### 3. Update `IdempotencyRecord` (backend/src/domain/idempotency/mod.rs)

Add `mutation_type` field:

```rust
/// Stored idempotency record linking a key to its payload and response.
#[derive(Debug, Clone)]
pub struct IdempotencyRecord {
    /// The idempotency key provided by the client.
    pub key: IdempotencyKey,
    /// The type of mutation this record protects.
    pub mutation_type: MutationType,
    /// SHA-256 hash of the canonicalized request payload.
    pub payload_hash: PayloadHash,
    /// Snapshot of the original response to replay.
    pub response_snapshot: serde_json::Value,
    /// User who made the original request.
    pub user_id: UserId,
    /// When the record was created.
    pub created_at: DateTime<Utc>,
}
```

### 4. Rename port trait (backend/src/domain/ports/)

Rename `idempotency_store.rs` to `idempotency_repository.rs`:

```rust
/// Port for idempotency record storage and retrieval.
///
/// Implementations provide durable storage for idempotency records, enabling
/// safe request retries by detecting duplicate requests and replaying previous
/// responses. The repository supports multiple mutation types, allowing keys
/// to be scoped per operation kind.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait IdempotencyRepository: Send + Sync {
    /// Look up an idempotency key for a specific user and mutation type.
    ///
    /// The lookup is scoped to the given user and mutation type to prevent
    /// cross-user or cross-operation key reuse.
    async fn lookup(
        &self,
        key: &IdempotencyKey,
        user_id: &UserId,
        mutation_type: MutationType,
        payload_hash: &PayloadHash,
    ) -> Result<IdempotencyLookupResult, IdempotencyRepositoryError>;

    /// Store an idempotency record.
    async fn store(
        &self,
        record: &IdempotencyRecord,
    ) -> Result<(), IdempotencyRepositoryError>;

    /// Remove records older than the specified TTL.
    async fn cleanup_expired(
        &self,
        ttl: Duration,
    ) -> Result<u64, IdempotencyRepositoryError>;
}
```

Update error enum to `IdempotencyRepositoryError` for consistency.

### 5. Create Diesel migration

Create `backend/migrations/<timestamp>_add_mutation_type_to_idempotency_keys/`:

`up.sql`:

```sql
-- Add mutation_type column to idempotency_keys table.
-- This enables the same idempotency key to be used across different
-- mutation types without collision.

ALTER TABLE idempotency_keys
ADD COLUMN mutation_type TEXT NOT NULL DEFAULT 'routes';

-- Add CHECK constraint for known mutation types.
ALTER TABLE idempotency_keys
ADD CONSTRAINT chk_mutation_type CHECK (
    mutation_type IN ('routes', 'notes', 'progress', 'preferences', 'bundles')
);

-- Drop the existing primary key and recreate with mutation_type.
-- The existing composite key is (key, user_id); we extend to
-- (key, user_id, mutation_type).
ALTER TABLE idempotency_keys DROP CONSTRAINT idempotency_keys_pkey;

ALTER TABLE idempotency_keys
ADD PRIMARY KEY (key, user_id, mutation_type);

-- Create index for lookups by user and mutation type.
CREATE INDEX idx_idempotency_keys_user_mutation
ON idempotency_keys (user_id, mutation_type);
```

`down.sql`:

```sql
-- Revert mutation_type addition.
DROP INDEX IF EXISTS idx_idempotency_keys_user_mutation;

ALTER TABLE idempotency_keys DROP CONSTRAINT idempotency_keys_pkey;
ALTER TABLE idempotency_keys ADD PRIMARY KEY (key, user_id);

ALTER TABLE idempotency_keys DROP CONSTRAINT IF EXISTS chk_mutation_type;
ALTER TABLE idempotency_keys DROP COLUMN IF EXISTS mutation_type;
```

### 6. Update Diesel schema (backend/src/outbound/persistence/schema.rs)

Update the table definition:

```rust
diesel::table! {
    idempotency_keys (key, user_id, mutation_type) {
        key -> Uuid,
        user_id -> Uuid,
        mutation_type -> Text,
        payload_hash -> Bytea,
        response_snapshot -> Jsonb,
        created_at -> Timestamptz,
    }
}
```

### 7. Update Diesel models (backend/src/outbound/persistence/models.rs)

Add `mutation_type` to `IdempotencyKeyRow` and `NewIdempotencyKeyRow`.

### 8. Rename and update adapter (backend/src/outbound/persistence/)

Rename `diesel_idempotency_store.rs` to `diesel_idempotency_repository.rs`:

- Rename struct to `DieselIdempotencyRepository`.
- Update `lookup` to filter by `mutation_type`.
- Update `store` to include `mutation_type`.
- Update error mapping to use `IdempotencyRepositoryError`.

### 9. Update domain service (backend/src/domain/route_submission/mod.rs)

- Update imports to use `IdempotencyRepository`.
- Pass `MutationType::Routes` to lookup and store calls.
- No behaviour change for existing route submission flow.

### 10. Update fixture implementation

Rename `FixtureIdempotencyStore` to `FixtureIdempotencyRepository` and update
method signatures.

### 11. Update HTTP state and server wiring

- Update `HttpState` field type if necessary.
- Update `main.rs` or `server/mod.rs` wiring.

### 12. BDD tests

Create
`backend/tests/features/shared_idempotency_repository.feature`:

```gherkin
Feature: Shared idempotency repository

  Background:
    Given a postgres-backed idempotency repository

  Scenario: Route submission idempotency works with mutation type
    Given a valid route submission request
    When the request is submitted with a fresh idempotency key
    Then the response status is 202 Accepted
    And the idempotency record has mutation type "routes"

  Scenario: Same key can be used for different mutation types
    Given an idempotency key stored for routes
    When the same key is used for a notes mutation
    Then the notes mutation is treated as new
    And no conflict is raised

  Scenario: Conflicting payload within same mutation type is rejected
    Given an idempotency key stored for routes
    When a different payload is submitted with the same key for routes
    Then the response status is 409 Conflict

  Scenario: Matching payload replays response
    Given an idempotency key stored for routes
    When the same payload is submitted with the same key for routes
    Then the original response is replayed

  Scenario: Expired records are cleaned up
    Given an idempotency record created 25 hours ago
    When cleanup is run with a 24-hour TTL
    Then the record is removed
```

Create `backend/tests/shared_idempotency_repository_bdd.rs`:

- Follow patterns from `ports_behaviour.rs`.
- Use `pg_embedded_setup_unpriv::TestCluster` fixture.
- Implement step definitions for all scenarios.

### 13. Contract tests for `IdempotencyRepository`

Create `backend/tests/idempotency_repository_contract.rs`:

- Test `lookup` returns `NotFound` for unknown keys.
- Test `lookup` respects mutation type isolation.
- Test `store` persists record with mutation type.
- Test `lookup` returns `MatchingPayload` when hash matches.
- Test `lookup` returns `ConflictingPayload` when hash differs.
- Test `cleanup_expired` removes old records.
- Test `cleanup_expired` respects mutation type.

### 14. Unit tests

Add to domain module tests:

- `MutationType::as_str` correctness.
- `MutationType` serde round-trip.
- `IdempotencyConfig::from_env` with and without variable.
- `IdempotencyConfig::default` returns 24 hours.

### 15. Documentation updates

Update `docs/wildside-backend-architecture.md`:

- Add design decision for shared `IdempotencyRepository`.
- Update driven ports section to reflect renamed port.
- Add `MutationType` to domain types documentation.
- Document `IDEMPOTENCY_TTL_HOURS` configuration.

Update `docs/backend-roadmap.md`:

- Mark the shared `IdempotencyRepository` task as done.

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

1. **Port renamed and generalized**:
   - `IdempotencyStore` renamed to `IdempotencyRepository`.
   - Port trait accepts `MutationType` parameter.
   - Error enum renamed to `IdempotencyRepositoryError`.

2. **Mutation type support**:
   - `MutationType` enum with variants: `Routes`, `Notes`, `Progress`,
     `Preferences`, `Bundles`.
   - `IdempotencyRecord` includes `mutation_type` field.
   - Database schema includes `mutation_type` column in composite key.

3. **Configurable TTL**:
   - `IdempotencyConfig` loads from `IDEMPOTENCY_TTL_HOURS`.
   - Default TTL is 24 hours.
   - `cleanup_expired` uses configured TTL.

4. **Backward compatibility**:
   - Existing route submission behaviour unchanged.
   - Existing records default to `mutation_type = 'routes'`.
   - Existing tests continue to pass.

5. **Testing**:
   - Unit tests cover `MutationType` and `IdempotencyConfig`.
   - Contract tests validate port semantics with mutation types.
   - BDD tests cover happy and unhappy paths against embedded PostgreSQL.

6. **Documentation and quality**:
   - Architecture documentation updated.
   - Roadmap task marked complete.
   - All quality gates pass.

## Idempotence and Recovery

- Domain types are pure and testable.
- Diesel migrations are tracked in `__diesel_schema_migrations`, preventing
  re-execution.
- Store operations use UPSERT semantics where appropriate.
- If a command fails, fix the issue and re-run only the failed command.
- The migration is additive (adding column with default), so rollback is safe.

## Artifacts and Notes

Keep log files created by the `tee` commands until the work is complete:

- `/tmp/wildside-check-fmt.log`
- `/tmp/wildside-lint.log`
- `/tmp/wildside-test.log`
- `/tmp/wildside-markdownlint.log`

## Interfaces and Dependencies

Updated domain types in `backend/src/domain/idempotency/mod.rs`:

```rust
/// The type of mutation protected by idempotency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationType {
    Routes,
    Notes,
    Progress,
    Preferences,
    Bundles,
}

/// Configuration for idempotency behaviour.
#[derive(Debug, Clone)]
pub struct IdempotencyConfig {
    pub ttl: Duration,
}

/// Stored idempotency record.
pub struct IdempotencyRecord {
    pub key: IdempotencyKey,
    pub mutation_type: MutationType,
    pub payload_hash: PayloadHash,
    pub response_snapshot: serde_json::Value,
    pub user_id: UserId,
    pub created_at: DateTime<Utc>,
}
```

Updated port trait in `backend/src/domain/ports/idempotency_repository.rs`:

```rust
#[async_trait]
pub trait IdempotencyRepository: Send + Sync {
    async fn lookup(
        &self,
        query: &IdempotencyLookupQuery,
    ) -> Result<IdempotencyLookupResult, IdempotencyRepositoryError>;

    async fn store(
        &self,
        record: &IdempotencyRecord,
    ) -> Result<(), IdempotencyRepositoryError>;

    async fn cleanup_expired(
        &self,
        ttl: Duration,
    ) -> Result<u64, IdempotencyRepositoryError>;
}

/// Query parameters for looking up an idempotency key.
pub struct IdempotencyLookupQuery {
    pub key: IdempotencyKey,
    pub user_id: UserId,
    pub mutation_type: MutationType,
    pub payload_hash: PayloadHash,
}
```

Updated `RouteSubmissionServiceImpl` usage:

```rust
// Before
self.idempotency_store.lookup(&key, &user_id, &hash).await

// After
let query = IdempotencyLookupQuery::new(key, user_id, MutationType::Routes, hash);
self.idempotency_repository.lookup(&query).await
```
