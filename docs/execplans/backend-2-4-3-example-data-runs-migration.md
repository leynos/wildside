# Add example_data_runs migration and repository helper

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

This document must be maintained in accordance with
`/home/leynos/.claude/skills/execplans/PLANS.md`.

## Purpose / Big Picture

Task 2.4.3 from `docs/backend-roadmap.md` requires adding a migration for the
`example_data_runs` table and a repository helper that guards seeding to occur
only once per seed name. After this change, the backend will have the
persistence layer needed to track which example data seeds have been applied,
preventing duplicate seeding on concurrent startups or restarts.

Observable outcome: The migration creates the `example_data_runs` table with
appropriate schema. The repository port and adapter allow inserting a seed
marker with `INSERT ... ON CONFLICT DO NOTHING` semantics. Unit tests validate
error mapping, and integration tests with embedded Postgres verify idempotent
seeding behaviour.

## Constraints

- **Hexagonal architecture**: The repository must follow the ports-and-adapters
  pattern established in `docs/wildside-backend-architecture.md`. The port
  trait lives in `backend/src/domain/ports/`, the adapter in
  `backend/src/outbound/persistence/`.
- **No domain type coupling**: The `example-data` crate remains independent of
  backend domain types. The repository operates on simple types (strings,
  integers) rather than domain newtypes.
- **Existing schema conventions**: The migration follows the established naming
  pattern (`YYYY-MM-DD-HHMMSS_description`) and uses the same trigger and
  timestamp patterns as other tables.
- **Error handling**: Use `define_port_error!` macro for domain errors, mapping
  pool and Diesel errors to domain variants.
- **Testing requirements**: Tests must use `rstest` for unit tests and
  `rstest-bdd` v0.3.2 for behavioural tests. Integration tests use
  `pg-embedded-setup-unpriv` for isolated Postgres instances.

## Tolerances (Exception Triggers)

- **Scope**: If implementation requires changes to more than 15 files or 600
  lines of code (net), stop and escalate.
- **Interface**: If existing public API signatures must change, stop and
  escalate.
- **Dependencies**: No new external dependencies are expected. If one is
  required, stop and escalate.
- **Iterations**: If tests still fail after 3 fix attempts, stop and escalate.
- **Ambiguity**: If the table schema or repository interface could reasonably
  be designed multiple ways with material impact, present options.

## Risks

- Risk: Migration syntax errors or missing down migration
  Severity: low
  Likelihood: low
  Mitigation: Follow existing migration patterns exactly; test with diesel
  migration run/revert cycle.

- Risk: Incorrect ON CONFLICT behaviour leading to race conditions
  Severity: medium
  Likelihood: low
  Mitigation: Write explicit integration test verifying idempotent insert
  returns correct status.

- Risk: Schema.rs and models.rs drift from migration
  Severity: low
  Likelihood: low
  Mitigation: Manually verify schema.rs matches migration columns; run full
  test suite.

## Progress

- [x] (2026-01-16) Stage A: Create migration for `example_data_runs` table
- [x] (2026-01-16) Stage B: Add Diesel schema and models
- [x] (2026-01-16) Stage C: Define port trait with error types
- [x] (2026-01-16) Stage D: Implement Diesel adapter
- [x] (2026-01-16) Stage E: Add unit tests for error mapping
- [x] (2026-01-16) Stage F: Add integration tests with embedded Postgres
- [x] (2026-01-16) Stage G: Add BDD behavioural tests
- [x] (2026-01-16) Stage H: Update architecture documentation
- [x] (2026-01-16) Stage I: Mark roadmap task as done
- [x] (2026-01-16) Stage J: Run full validation suite

## Surprises & Discoveries

- Observation: Backend uses rstest-bdd v0.2.0, not v0.3.2 as stated in the task
  Evidence: Cargo.toml shows `rstest_bdd_macros = "0.2.0"`
  Impact: Used v0.2.0 patterns for BDD tests; no issues encountered.

- Observation: Type inference issues with `handle_cluster_setup_failure` in BDD tests
  Evidence: Compiler error `cannot infer type of the type parameter T`
  Impact: Required explicit type annotation `let _: Option<()>` when discarding result.

- Observation: ExampleDataRunRow struct flagged as dead code
  Evidence: Clippy warning about unused struct
  Impact: Added `#[expect(dead_code)]` annotation; the struct will be used when
  seed audit/query functionality is added in future tasks.

## Decision Log

- Decision: Use `seed_key TEXT PRIMARY KEY` rather than a composite key
  Rationale: The design document specifies seed_key as the sole identifier;
  each seed name is unique and sufficient for once-only guard.
  Date/Author: 2026-01-16 / Plan author

- Decision: Return `SeedingResult` enum from repository method
  Rationale: The caller needs to distinguish between "seed applied" and "seed
  already exists" without treating the latter as an error. An enum is cleaner
  than Option or bool.
  Date/Author: 2026-01-16 / Plan author

## Outcomes & Retrospective

Implementation completed successfully. All deliverables met:

1. **Migration**: Created `2026-01-16-000000_create_example_data_runs` with
   up.sql and down.sql following existing conventions.

2. **Port trait**: `ExampleDataRunsRepository` with `try_record_seed` and
   `is_seeded` methods, using `SeedingResult` enum to distinguish outcomes.

3. **Adapter**: `DieselExampleDataRunsRepository` implementing idempotent
   seeding via `INSERT ... ON CONFLICT DO NOTHING` with correct row-count
   interpretation.

4. **Testing**:
   - 2 unit tests for error mapping in adapter module
   - 5 integration tests with embedded Postgres
   - 4 BDD scenarios covering all seeding guard behaviours

5. **Documentation**: Updated architecture doc with `ExampleDataRunsRepository`
   in driven ports section; marked roadmap task 2.4.3 as complete.

**Validation**: 468 tests passed, 1 skipped. All lints and formatting checks
pass.

**Lessons learned**:

- The existing test infrastructure (pg-embedded-setup-unpriv) works well for
  new repository tests; copying established patterns reduces friction.
- BDD tests in this repo use rstest-bdd v0.2.0 syntax; verify dependency
  versions before starting implementation.
- Generic helper functions like `handle_cluster_setup_failure<T>` may need
  explicit type annotations when result is discarded.

## Context and Orientation

The Wildside backend implements a hexagonal architecture where domain logic is
isolated from infrastructure concerns through ports (traits) and adapters
(implementations).

Key paths:

- Domain ports: `backend/src/domain/ports/`
- Persistence adapters: `backend/src/outbound/persistence/`
- Migrations: `backend/migrations/`
- Schema definitions: `backend/src/outbound/persistence/schema.rs`
- Row models: `backend/src/outbound/persistence/models.rs`
- Integration tests: `backend/tests/`

The `example-data` crate (`crates/example-data/`) provides deterministic user
generation but does not persist data. This task adds the persistence layer for
tracking which seeds have been applied.

Related documents:

- `docs/backend-sample-data-design.md` - Design specification
- `docs/wildside-backend-architecture.md` - Architecture reference
- `docs/pg-embedded-setup-unpriv-users-guide.md` - Testing with embedded Postgres

## Plan of Work

### Stage A: Create migration

Create `backend/migrations/2026-01-16-000000_create_example_data_runs/` with
`up.sql` and `down.sql`.

The table schema (from design doc):

- `seed_key TEXT PRIMARY KEY` - The seed name, unique identifier
- `seeded_at TIMESTAMPTZ NOT NULL DEFAULT NOW()` - When seeding occurred
- `user_count INTEGER NOT NULL` - Number of users created
- `seed BIGINT NOT NULL` - The RNG seed value used

No foreign keys, triggers, or indices required (simple marker table).

### Stage B: Add Diesel schema and models

In `backend/src/outbound/persistence/schema.rs`, add:

    diesel::table! {
        example_data_runs (seed_key) {
            seed_key -> Text,
            seeded_at -> Timestamptz,
            user_count -> Int4,
            seed -> Int8,
        }
    }

Update `allow_tables_to_appear_in_same_query!` macro to include the new table.

In `backend/src/outbound/persistence/models.rs`, add:

- `ExampleDataRunRow` - Queryable struct for SELECT
- `NewExampleDataRunRow` - Insertable struct for INSERT

### Stage C: Define port trait

Create `backend/src/domain/ports/example_data_runs_repository.rs`:

    define_port_error! {
        pub enum ExampleDataRunsError {
            Connection { message: String } => "...",
            Query { message: String } => "...",
        }
    }

    /// Result of attempting to record a seed run.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SeedingResult {
        /// Seed was newly recorded; proceed with seeding.
        Applied,
        /// Seed was already recorded; skip seeding.
        AlreadySeeded,
    }

    #[async_trait]
    pub trait ExampleDataRunsRepository: Send + Sync {
        /// Attempt to record a seed run. Returns `Applied` if the record was
        /// inserted, `AlreadySeeded` if it already exists.
        async fn try_record_seed(
            &self,
            seed_key: &str,
            user_count: i32,
            seed: i64,
        ) -> Result<SeedingResult, ExampleDataRunsError>;

        /// Check if a seed has already been applied.
        async fn is_seeded(&self, seed_key: &str) -> Result<bool, ExampleDataRunsError>;
    }

Export from `backend/src/domain/ports/mod.rs`.

### Stage D: Implement Diesel adapter

Create `backend/src/outbound/persistence/diesel_example_data_runs_repository.rs`:

- `DieselExampleDataRunsRepository` struct with `DbPool`
- `map_pool_error` and `map_diesel_error` helper functions
- Implement `try_record_seed` using:

      INSERT INTO example_data_runs (seed_key, user_count, seed)
      VALUES ($1, $2, $3)
      ON CONFLICT (seed_key) DO NOTHING

  Check affected rows: 1 = Applied, 0 = AlreadySeeded.

- Implement `is_seeded` with simple SELECT EXISTS query.

Export from `backend/src/outbound/persistence/mod.rs`.

### Stage E: Add unit tests for error mapping

In the adapter module, add `#[cfg(test)]` block with rstest tests:

- `pool_error_maps_to_connection_error`
- `diesel_error_maps_to_query_error`

Follow the pattern in `diesel_user_repository.rs`.

### Stage F: Add integration tests with embedded Postgres

Create `backend/tests/diesel_example_data_runs_repository.rs`:

Use the established pattern from `diesel_user_repository.rs`:

- `TestContext` with runtime, repository, database
- `diesel_world` fixture with `pg-embed-setup-unpriv`
- Test cases:
  - `try_record_seed_returns_applied_on_first_insert`
  - `try_record_seed_returns_already_seeded_on_duplicate`
  - `is_seeded_returns_false_for_unknown_seed`
  - `is_seeded_returns_true_after_recording`

### Stage G: Add BDD behavioural tests

Create `backend/tests/example_data_runs_bdd.rs` and
`backend/tests/features/example_data_runs.feature`:

Feature: Example data seeding guard

  Scenario: First seed attempt succeeds
    Given a fresh database
    When a seed is recorded for "mossy-owl"
    Then the result is "applied"

  Scenario: Duplicate seed attempt is detected
    Given a database with seed "mossy-owl" already recorded
    When a seed is recorded for "mossy-owl"
    Then the result is "already seeded"

  Scenario: Different seeds are independent
    Given a database with seed "mossy-owl" already recorded
    When a seed is recorded for "clever-fox"
    Then the result is "applied"

### Stage H: Update architecture documentation

Add a brief entry to `docs/wildside-backend-architecture.md` under the
appropriate section documenting the `ExampleDataRunsRepository` port and its
purpose.

### Stage I: Mark roadmap task as done

Update `docs/backend-roadmap.md` to mark task 2.4.3 as complete:

    - [x] 2.4.3. Add the `example_data_runs` migration plus a repository helper
      to guard seeding once per seed name.

### Stage J: Run full validation suite

    make check-fmt && make lint && make test

## Concrete Steps

All commands run from repository root:
`/data/leynos/Projects/wildside.worktrees/backend-2-4-3-example-data-runs-migration`

### Stage A: Migration commands

    mkdir -p backend/migrations/2026-01-16-000000_create_example_data_runs

Create `up.sql`:

    -- Create example_data_runs table for tracking applied demo data seeds.
    -- Used by the example-data feature to ensure once-only seeding.

    CREATE TABLE example_data_runs (
        seed_key TEXT PRIMARY KEY,
        seeded_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
        user_count INTEGER NOT NULL,
        seed BIGINT NOT NULL
    );

    -- Add comment for documentation
    COMMENT ON TABLE example_data_runs IS
        'Tracks applied example data seeds to prevent duplicate seeding';

Create `down.sql`:

    DROP TABLE IF EXISTS example_data_runs;

### Stage B-J: Implementation files

Create/edit files as described in Plan of Work.

## Validation and Acceptance

Quality criteria:

- Tests: All existing tests pass; new tests for the repository pass
- Lint/typecheck: `make lint` succeeds with no warnings
- Formatting: `make check-fmt` succeeds

Quality method:

    make check-fmt && make lint && make test

Expected output includes:

- No formatting errors
- No clippy warnings
- All tests pass (current count ~450 + new tests)

New tests should be visible in output:

- `diesel_example_data_runs_repository::try_record_seed_returns_applied_on_first_insert`
- `diesel_example_data_runs_repository::try_record_seed_returns_already_seeded_on_duplicate`
- `diesel_example_data_runs_repository::is_seeded_returns_false_for_unknown_seed`
- `diesel_example_data_runs_repository::is_seeded_returns_true_after_recording`
- BDD scenarios from `example_data_runs_bdd.rs`

## Idempotence and Recovery

All changes are additive (new files, new table). If implementation fails
partway:

- Migration can be reverted with `diesel migration revert`
- New source files can be deleted
- Git can restore to clean state

The migration itself is idempotent: running it twice has no effect after the
first application (table already exists).

## Artifacts and Notes

Example migration up.sql content:

    CREATE TABLE example_data_runs (
        seed_key TEXT PRIMARY KEY,
        seeded_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
        user_count INTEGER NOT NULL,
        seed BIGINT NOT NULL
    );

Example port trait signature:

    pub trait ExampleDataRunsRepository: Send + Sync {
        async fn try_record_seed(
            &self,
            seed_key: &str,
            user_count: i32,
            seed: i64,
        ) -> Result<SeedingResult, ExampleDataRunsError>;

        async fn is_seeded(
            &self,
            seed_key: &str,
        ) -> Result<bool, ExampleDataRunsError>;
    }

## Interfaces and Dependencies

New files to create:

- `backend/migrations/2026-01-16-000000_create_example_data_runs/up.sql`
- `backend/migrations/2026-01-16-000000_create_example_data_runs/down.sql`
- `backend/src/domain/ports/example_data_runs_repository.rs`
- `backend/src/outbound/persistence/diesel_example_data_runs_repository.rs`
- `backend/tests/diesel_example_data_runs_repository.rs`
- `backend/tests/example_data_runs_bdd.rs`
- `backend/tests/features/example_data_runs.feature`

Files to modify:

- `backend/src/domain/ports/mod.rs` (add export)
- `backend/src/outbound/persistence/mod.rs` (add export)
- `backend/src/outbound/persistence/schema.rs` (add table definition)
- `backend/src/outbound/persistence/models.rs` (add row structs)
- `docs/wildside-backend-architecture.md` (add documentation)
- `docs/backend-roadmap.md` (mark task complete)

Dependencies (all existing in workspace):

- `diesel` - ORM and query builder
- `diesel-async` - Async Diesel support
- `async-trait` - Async trait support
- `rstest` - Test fixtures
- `rstest-bdd` v0.3.2 - BDD testing
- `pg-embedded-setup-unpriv` - Embedded Postgres for tests
