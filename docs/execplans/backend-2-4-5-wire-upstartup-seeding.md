# Wire Startup Example Data Seeding Behind Feature Flags

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and `Outcomes &
Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

There is no `PLANS.md` in this repository, so this ExecPlan is the sole
execution reference.

## Purpose / Big Picture

Enable deterministic demo data seeding at backend startup when explicitly
allowed by the `example-data` feature flag and `ortho-config` settings. The
seeding process must log whether it ran or was skipped, and must be safe to
re-run without duplicating data. Success is observable when:

- Startup logs state whether seeding was applied or skipped, including the seed
  name and user count when applicable.
- Seeding is disabled by default and does nothing when the feature is not
  compiled or configuration toggles are off.
- Unit tests (`rstest`) and behavioural tests (`rstest-bdd` v0.4.0) cover happy
  and unhappy paths, including configuration failures and already-seeded
  scenarios.
- Integration tests use `pg-embedded-setup-unpriv` so Postgres-backed seeding
  works locally.
- `docs/wildside-backend-architecture.md` records the seeding decisions, and
  `docs/backend-roadmap.md` marks task 2.4.5 as done.
- `make check-fmt`, `make lint`, and `make test` succeed.

## Constraints

- Preserve hexagonal boundaries: inbound adapters may only call domain services;
  outbound persistence stays behind ports. Avoid direct Diesel access from
  `main.rs` or HTTP handlers.
- The seeding flow must be gated behind the `example-data` Cargo feature and
  `ortho-config` settings, matching the configuration conventions already used
  by the backend.
- Use `tracing` for structured logging, and never log generated user content.
- Keep modules under 400 lines; extract helpers where required.
- All new modules must start with `//!` module docs and keep en-GB spelling.
- Tests must use `rstest` and `rstest-bdd` v0.4.0, and Postgres-backed tests
  must use the `pg-embedded-setup-unpriv` helpers.
- Documentation updates must follow `docs/documentation-style-guide.md` and be
  formatted via `make fmt`.

## Tolerances (Exception Triggers)

- Scope: if implementation requires changes to more than 18 files or 900 lines
  of code (net), stop and escalate.
- Dependencies: if any new dependency beyond `ortho-config` (and any required
  transitive updates for `rstest-bdd` v0.4.0) is needed, stop and escalate.
- Interfaces: if a public API signature must change outside the seeding surface,
  stop and escalate.
- Iterations: if tests still fail after 3 fix attempts, stop and escalate.
- Ambiguity: if `ortho-config` naming conventions are unclear or conflict with
  existing configuration patterns, stop and request clarification.

## Risks

Risk: `ortho-config` integration is new in this repo and may require extra
scaffolding for settings files or environment mapping. Severity: medium
Likelihood: medium Mitigation: audit existing configuration patterns first; keep
the seeding config isolated and documented.

Risk: `rstest-bdd` v0.4.0 introduces API changes relative to v0.3.2. Severity:
medium Likelihood: medium Mitigation: update dev dependencies early and adjust
existing macros or test helpers as needed, limiting changes to seeding-related
tests only.

Risk: Transactional seeding may be hard to express through existing ports.
Severity: medium Likelihood: medium Mitigation: introduce a dedicated seeding
port/adapter that owns the transaction boundary, keeping domain logic pure.

## Progress

- [x] (2026-02-03) ExecPlan approved; implementation started.
- [x] (2026-02-03) Audit existing seeding-related code, configuration modules,
      and tests to confirm current behaviour and gaps.
- [x] (2026-02-03) Add the `example-data` feature flag and `ortho-config`
      settings model for example data seeding.
- [x] (2026-02-03) Implement domain-level seeding orchestration and outbound
      adapter wiring behind ports, including logging decisions.
- [x] (2026-02-03) Add unit tests (`rstest`) for config and seeding logic.
- [x] (2026-02-03) Add behavioural tests (`rstest-bdd` v0.4.0) with embedded
      Postgres covering applied/skip/error paths.
- [x] (2026-02-03) Update architecture documentation and mark roadmap task 2.4.5
      complete.
- [x] (2026-02-03) Run `make check-fmt`, `make lint`, and `make test` with
      logged output and confirm success.

## Surprises & Discoveries

Observation (2026-02-03): `ortho_config` derive macros require `clap` to be a
direct dependency for generated CLI parsing helpers. Evidence: `cargo clippy`
failed with unresolved `clap` imports until `clap` was added behind the
`example-data` feature. Impact: add `clap` as a feature-gated dependency in
`backend/Cargo.toml`.

Observation (2026-02-03): Whitaker's `no_std_fs_operations` lint rejected
`std::fs::read_to_string` in startup seeding; switching to capability-based
`cap_std::fs::Dir` reads aligned with the repo's filesystem policy.

## Decision Log

Decision (2026-02-03): Introduce `ExampleDataSeedRepository` to own the
transaction boundary for recording seed runs and upserting users/preferences,
keeping the domain seeding service pure. Configuration uses `ortho_config`
prefix `EXAMPLE_DATA_`, with `count` mapped to config file key
`user_count` and defaults for the seed name (`mossy-owl`) and registry path
(`backend/fixtures/example-data/seeds.json`). Rationale: align with the
sample-data design while keeping seeding optional and testable across env and
config sources.

## Outcomes & Retrospective

Outcome (2026-02-03):

- Startup seeding is fully gated behind the `example-data` feature and
  `ortho-config` settings, with capability-based registry reads and structured
  logging for applied and skipped runs.
- Unit tests (`rstest`) and behavioural tests (`rstest-bdd` v0.4.0) cover config
  parsing, successful seeding, idempotent skips, and missing seed failures with
  embedded Postgres.
- Quality gates passed: `make check-fmt`, `make lint`, and `make test`.

## Context and Orientation

Key locations (repository-relative):

- `docs/backend-roadmap.md`: phase 2.4.5 task definition and acceptance.
- `docs/backend-sample-data-design.md`: seeding design, configuration fields,
  and startup requirements.
- `backend/src/main.rs`: application bootstrap and server startup.
- `backend/src/server/mod.rs`: server wiring and dependency injection.
- `backend/src/server/config.rs`: server configuration builder.
- `backend/src/domain/ports/example_data_runs_repository.rs`: port for seed run
  tracking.
- `backend/src/outbound/persistence/diesel_example_data_runs_repository.rs`:
  Postgres adapter for seed run tracking.
- `backend/src/outbound/persistence/diesel_user_repository.rs` and
  `backend/src/outbound/persistence/diesel_user_preferences_repository.rs`:
  persistence adapters used by seeding.
- `crates/example-data/`: deterministic user generation and registry parsing.
- `backend/fixtures/example-data/seeds.json`: seed registry file.
- `backend/tests/support/pg_embed.rs`: embedded Postgres bootstrapping.
- `docs/wildside-backend-architecture.md`: location for new design decision
  entry.
- `docs/rust-testing-with-rstest-fixtures.md` and
  `docs/rstest-bdd-users-guide.md`: testing conventions and BDD wiring.
- `docs/pg-embed-setup-unpriv-users-guide.md`: Postgres bootstrap guidance.

Terminology (plain-language):

- _Feature flag_: Cargo feature (`example-data`) that gates compilation and
  runtime behaviour.
- _Seed registry_: JSON file listing seed names and deterministic seed values.
- _Seeding applied_: seed record inserted and demo data written.
- _Seeding skipped_: seeding did not run because it was disabled, already ran,
  or prerequisites were missing (e.g., no database URL).

## Plan of Work

Stage A: Confirm current state and requirements.

- Read `docs/backend-sample-data-design.md` and the relevant port/adapters to
  align on the expected configuration fields, transactional guarantees, and
  logging behaviour.
- Inspect existing configuration parsing to decide how `ortho-config` should be
  integrated and whether any existing settings files exist.
- Verify the current `rstest-bdd` version and note any API differences with
  v0.4.0 that could affect new tests.

Stage B: Configuration and feature flag scaffolding.

- Add an `example-data` Cargo feature to `backend/Cargo.toml`, with optional
  dependencies on `example-data` and `ortho-config` gated behind it.
- Define an `ExampleDataSettings` struct (and supporting loader) that uses
  `ortho-config` to read `example_data.enabled`, `example_data.seed_name`,
  `example_data.user_count` (optional override), and
  `example_data.registry_path`. Ensure env overrides are wired as documented in
  `docs/backend-sample-data-design.md`.
- Provide a minimal error type for configuration failures, with clear messages
  that can surface during startup.

Stage C: Seeding orchestration behind ports.

- Introduce a domain-level seeding service (e.g.,
  `backend/src/domain/example_data/service.rs`) that accepts a parsed
  `SeedRegistry`, a selected `SeedDefinition`, and ports for `UserRepository`,
  `UserPreferencesRepository`, and `ExampleDataRunsRepository` (or a dedicated
  seeding port if needed to own the transaction boundary).
- Ensure the seeding service checks/records the seed via
  `ExampleDataRunsRepository`, generates deterministic users using
  `example-data`, inserts users and preferences via repository ports, and
  returns `Applied` vs `AlreadySeeded` so callers can log accordingly.
- If transactions are required to satisfy the design, implement a dedicated
  outbound adapter that owns the transaction boundary and is surfaced via a new
  port in `backend/src/domain/ports`.

Stage D: Startup wiring and logging.

- In `backend/src/main.rs`, when the `example-data` feature is enabled: load
  `ExampleDataSettings` using `ortho-config`, log and skip when disabled, and
  when enabled build the database pool from `DATABASE_URL` (or configured
  settings) before invoking the seeding service. Log `seeding applied` vs
  `seeding skipped`, including `seed_key` and `user_count` when available.
- Preserve existing startup behaviour when the feature is not enabled.

Stage E: Tests.

- Unit tests (`rstest`) should cover configuration parsing (enabled/disabled
  paths, missing required fields, invalid registry path, invalid seed name,
  optional user count override) and seeding service outcomes (applied vs
  already-seeded, error mapping for configuration and persistence failures).
- Behavioural tests (`rstest-bdd` v0.4.0) should cover: seeding enabled and
  applied on a fresh database, seeding enabled but already recorded (skip log
  and no duplicate data), and seeding enabled but registry/seed missing
  (fail-fast). Use `pg-embedded-setup-unpriv` helpers
  (`backend/tests/support/pg_embed.rs`) for Postgres-backed flows.

Stage F: Documentation and roadmap.

- Update `docs/wildside-backend-architecture.md` with a design decision entry
  covering config source (`ortho-config`), feature flag gating, startup
  behaviour, and seeding skip vs applied logging semantics.
- Mark roadmap task 2.4.5 as done in `docs/backend-roadmap.md`.

Stage G: Validation and commits.

- Run `make check-fmt`, `make lint`, and `make test` with output captured via
  `tee` to `/tmp/$ACTION-$(get-project)-$(git branch --show).out`.
- Commit each logical change after its quality gates pass, following the
  project’s commit message format.

## Concrete Steps

1. Review relevant docs and existing seeding-related code paths:
`docs/backend-roadmap.md`, `docs/backend-sample-data-design.md`,
`backend/src/main.rs`, `backend/src/server/mod.rs`, and
`backend/src/domain/ports/example_data_runs_repository.rs`.

2. Add feature flag + dependencies in `backend/Cargo.toml` and wire
`example-data` + `ortho-config` behind that flag.

3. Implement `ExampleDataSettings` loader using `ortho-config` and add unit
tests using `rstest`.

4. Add domain seeding service + ports/adapters, ensuring transactional semantics
if required by the design document.

5. Wire startup seeding in `backend/src/main.rs` (feature-gated) and add
structured `tracing` logs for applied/skip paths.

6. Add `rstest-bdd` scenarios and feature file under `backend/tests/features/`
for startup seeding behaviour, using embedded Postgres helpers.

7. Update `docs/wildside-backend-architecture.md` and mark 2.4.5 complete in
`docs/backend-roadmap.md`.

8. Run quality gates: `make check-fmt | tee /tmp/check-fmt-$(get-project)-$(git
branch --show).out`, `make lint | tee /tmp/lint-$(get-project)-$(git branch
--show).out`, and `make test | tee /tmp/test-$(get-project)-$(git branch
--show).out`.

## Validation and Acceptance

Acceptance criteria:

- When compiled without `--features example-data`, startup logs indicate seeding
  is skipped due to feature gating (no seeding runs).
- When compiled with the feature and `example_data.enabled=false`, seeding is
  skipped and logged.
- When enabled and configured with a valid seed + registry, seeding inserts
  users and preferences, records the seed, and logs `applied` once.
- When enabled and the seed is already recorded, seeding logs `skipped` and
  inserts nothing new.
- Invalid configuration (missing registry or seed) fails fast with a clear
  error.
- All tests and quality gates pass: `make check-fmt`, `make lint`, and `make
  test`.

## Idempotence and Recovery

- Seeding should be safe to re-run: the seed guard prevents duplication and logs
  a skip when already applied.
- All steps should be re-runnable; when tests fail, re-run after fixes using the
  same `make` commands and `tee` logs.
- If a config or migration error is introduced, revert only the new seeding
  wiring and re-run the tests to confirm rollback.

## Artifacts and Notes

- Expected log examples (shape only, not exact text):

    {"level":"INFO","message":"example data seeding skipped","seed_key":"mossy-owl","reason":"disabled"}
    {"level":"INFO","message":"example data seeding applied","seed_key":"mossy-owl","user_count":12}

- Expected new files: `backend/tests/features/example_data_seeding.feature` and
  `backend/tests/example_data_seeding_bdd.rs` (or similar).

## Interfaces and Dependencies

- Add `example-data` Cargo feature to `backend/Cargo.toml` and gate the optional
  dependencies `example-data` (workspace crate) and `ortho-config` (new
  dependency).

- Proposed new API surfaces (names may adjust after audit) include
  `backend::domain::example_data::ExampleDataSettings` loaded via `ortho-config`
  and `backend::domain::example_data::ExampleDataSeeder` with a method such as:

        pub async fn seed(&self, registry: &SeedRegistry, seed: &SeedDefinition)
            -> Result<SeedingOutcome, ExampleDataSeedError>;

  The optional `backend::domain::ports::ExampleDataSeederRepository` is added
  only if a dedicated transactional adapter is required.

- Use existing ports where possible: `UserRepository`,
  `UserPreferencesRepository`, and `ExampleDataRunsRepository`.

- Tests should use `pg_embedded_setup_unpriv::TestCluster` helpers from
  `backend/tests/support/pg_embed.rs`, `rstest` fixtures for config and service
  unit tests, and `rstest-bdd` v0.4.0 macros for behavioural tests.

## Revision note

- Updated status to IN PROGRESS and noted approval/start of implementation.
