# Backend sample data design

## Purpose

Provide a deterministic, once-only database seeding path for demonstration
purposes. The feature introduces an `example-data` crate and a backend
`example-data` feature that populates users and preferences from
`docs/wildside-pwa-data-model.md` on startup, so the application can be
presented with realistic, repeatable data.

## Goals

- Generate believable example users with preferences and interests that match
  the data model.
- Seed exactly once per seed name, even with concurrent startups.
- Keep the generator deterministic by default, with named seeds and config
  overrides.
- Avoid coupling the generator to backend domain types.
- Ensure the feature is opt-in and disabled by default.

## Non-goals

- Seeding catalogue, route, POI, or map data.
- Writing a generic data fixture framework for all domains.
- Replacing ingestion tooling such as `ingest-osm`.

## Data model alignment

This design targets the `UserPreferences` shape in
`docs/wildside-pwa-data-model.md`:

- `userId`: UUID.
- `interestThemeIds`: UUID array.
- `safetyToggleIds`: UUID array.
- `unitSystem`: `metric` or `imperial`.
- `revision`: integer (seeded as initial revision).
- `updatedAt`: set by database default.

The existing backend tables `users` and `user_preferences` already support
these fields. Interest and safety descriptors are not yet modelled as separate
backend tables, so the seed registry stores their UUIDs and persists them as
arrays in `user_preferences`.

## Design overview

### New crate: `example-data`

Create `crates/example-data` as a workspace member. The crate exports plain
Rust structs and functions without referencing backend domain types. This
avoids circular dependencies and keeps the generator reusable in adapters or
tooling.

Proposed API:

- `SeedRegistry { version: u32, interest_theme_ids: Vec<Uuid>,
  safety_toggle_ids: Vec<Uuid>, seeds: Vec<SeedDefinition> }`
- `SeedDefinition { name: String, seed: u64, user_count: usize }`
- `ExampleUserSeed { id: Uuid, display_name: String,
  interest_theme_ids: Vec<Uuid>, safety_toggle_ids: Vec<Uuid>, unit_system:
  UnitSystemSeed }`
- `UnitSystemSeed` enum with `Metric` and `Imperial`.
- `generate_example_users(registry: &SeedRegistry, seed: &SeedDefinition)
  -> Vec<ExampleUserSeed>`

### Generation rules

- Use the `fake` crate to generate name-like tokens.
- Enforce display name validation to match `backend/src/domain/user.rs` by
  reusing the same rules that `DisplayName::new` applies. If the generator
  performs pre-validation, mirror the `display_name_regex` pattern from
  `backend/src/domain/user.rs` (`^[A-Za-z0-9_ ]+$`) and update it whenever the
  domain constraints change.
- Use a deterministic RNG seed sourced from the named seed definition.
- Select interest and safety IDs from the registry, so the data is stable and
  aligns with future descriptor tables.
- Prefer metric units, but include a minority of imperial selections to
  demonstrate unit switching.

### Seed registry file

Store the seed registry in JSON so it can be edited without recompiling. The
registry includes descriptor UUIDs and named seed definitions. Proposed path:
`backend/fixtures/example-data/seeds.json`.

Example structure:

    {
      "version": 1,
      "interestThemeIds": ["…"],
      "safetyToggleIds": ["…"],
      "seeds": [
        { "name": "mossy-owl", "seed": 2026, "userCount": 12 }
      ]
    }

### Named seeds and lexis

Each seed entry has a memorable name. The CLI tool uses the `lexis` crate to
suggest names when creating a new seed, keeping the identifiers stable and
human-friendly.

### Seed creation CLI

Provide a small CLI tool (for example an `example-data-seed` binary) that:

- Reads the registry JSON file.
- Generates a new seed entry with a `lexis`-generated name.
- Accepts optional overrides for `seed`, `userCount`, or `name`.
- Writes the updated registry back to disk.

## Once-only seeding

### Seed marker table

Add a migration for a new table `example_data_runs`:

- `seed_key` (text, primary key).
- `seeded_at` (timestamp with time zone).
- `user_count` (integer).
- `seed` (bigint).

`seed_key` equals the selected seed name. This table is the authoritative
guard. Seeding attempts insert a row for the `seed_key`. If the insert
succeeds, the seed proceeds. If the insert is ignored (because the key already
exists), the seed is skipped.

### Transactional seeding flow

The seed runner should operate within a single transaction:

1. Insert an `example_data_runs` row using `INSERT ... ON CONFLICT DO NOTHING`.
2. If no row is inserted, log `already seeded` and return.
3. Insert users (upsert by ID).
4. Insert user preferences (upsert by user ID, revision set to 1).

If any step fails, the transaction rolls back, leaving no partial data.

## Configuration and feature flags

Seeding runs only when the backend is compiled with the `example-data` feature
and configuration enables it. Configuration is loaded with `ortho-config` so it
can be sourced from a settings file and environment overrides. Ensure the key
naming matches existing backend `ortho-config` conventions before finalizing
the field names. Proposed config fields:

- `example_data.enabled`: boolean toggle.
- `example_data.seed_name`: seed name to load from the registry.
- `example_data.user_count`: optional override for the seed's default count.
- `example_data.registry_path`: path to the registry JSON.

Environment overrides (subject to the same naming conventions and mapping
rules `ortho-config` applies elsewhere):

- `EXAMPLE_DATA_ENABLED`
- `EXAMPLE_DATA_SEED_NAME`
- `EXAMPLE_DATA_COUNT`
- `EXAMPLE_DATA_REGISTRY_PATH`

If seeding is enabled but the registry or seed name cannot be resolved, startup
should fail with a clear error. If seeding is enabled but no database URL is
available, the seed should log a warning and exit without error.

## Startup wiring

The backend startup path should build a database pool from `DATABASE_URL` and
attach it to `ServerConfig` once the persistence layer is ready. When the
`example-data` feature is enabled, the seed runner should execute before
starting the HTTP server, so the demo data is ready for initial requests.

## Logging and observability

Log a single structured message indicating whether the seed was applied or
skipped, including the `seed_key` and `user_count`. Avoid logging generated
user content to keep logs clean.

## Error handling

- Map configuration and generator errors to descriptive messages.
- Treat database failures as fatal during startup when seeding is enabled.
- Keep errors scoped to the seeding feature to avoid polluting unrelated boot
  paths.

## Testing strategy

### Example-data crate unit tests

- Deterministic output for a fixed seed.
- Display name validity against backend constraints.
- Interest and safety selections remain within the registry.
- Registry parsing rejects invalid structures.

### Backend integration tests

Use `pg_embedded_setup_unpriv` to validate seeding behaviour:

- First run inserts users, preferences, and a marker row.
- Second run with the same seed key inserts nothing new.
- Seed marker values match the configuration (seed key, seed, count).

Tests should honour `SKIP_TEST_CLUSTER=1` to allow opt-out in restricted
environments.

### CLI tests

- Adding a seed updates the registry file with a unique `lexis` name.
- Existing seeds remain stable after update.

## Dependencies

- `fake = "2.10.0"` for name generation.
- `rand = "0.8.5"` for deterministic RNG.
- `lexis = "<latest stable>"` for memorable seed naming.
- `ortho-config = "<latest stable>"` for hierarchical configuration.
- Existing workspace `uuid` and `chrono` versions.

Dependency versions should use explicit values in `Cargo.toml` with the
repository's default caret semantics.

## Risks and mitigations

- **Seed drift**: registry UUIDs might diverge from future descriptor tables.
  Mitigation: document IDs and update them alongside descriptor ingestion.
- **Partial data**: failures during seeding could leave partial rows.
  Mitigation: run seeding in a transaction and rely on the marker table.
- **Registry churn**: manual edits could introduce invalid seeds.
  Mitigation: validate registry shape and provide the CLI for updates.
- **Feature misuse in production**: accidental enablement in production.
  Mitigation: default `example_data.enabled` to false and document expected
  usage in runbooks.

## Alternatives considered

- **Counting users to decide seeding**: rejected because it is ambiguous and
  unsafe with concurrent startups.
- **Embedding backend domain types in the generator**: rejected to avoid tight
  coupling and dependency cycles.

## Future considerations

- Store the registry in a dedicated fixture crate if more demo datasets are
  added.
- Support multiple registry versions for large design refreshes.

## Acceptance criteria

- Design document exists at `docs/backend-sample-data-design.md` and captures
  scope, workflow, configuration, and tests.
- Backend roadmap includes example-data phases and tasks.
- Documentation linting and formatting succeed.
