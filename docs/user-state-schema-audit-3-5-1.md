# User-state schema audit (roadmap 3.5.1)

This document records the roadmap 3.5.1 audit for login, users, profile, and
interests persistence coverage. It also captures migration decisions for
profile storage, interests storage, revision tracking, and stale-write conflict
handling.

## Audit scope and method

Audit scope:

- login persistence coverage
- users persistence coverage
- profile persistence coverage
- interests persistence coverage
- migration requirement decisions for profile/interests and concurrency support

Method:

1. Load migration-backed schema metadata through the domain port
   `SchemaSnapshotRepository`.
2. Evaluate coverage with the domain audit operation
   `audit_user_state_schema_coverage` and
   `UserStateSchemaAuditReport::evaluate`.
3. Validate outcomes with `rstest` unit tests and `rstest-bdd` behavioural
   tests against `pg-embedded-setup-unpriv` databases.

Primary implementation references are documented in the domain module and
behavioural suites[^1][^2][^3].

[^1]: `backend/src/domain/user_state_schema_audit.rs`
[^2]: `backend/tests/user_state_schema_audit_bdd.rs`
[^3]: `backend/tests/features/user_state_schema_audit.feature`

## Coverage findings

- Login persistence:
  Missing credential storage in schema (`MissingCredentialStorage`).
  Evidence: audit rules in `user_state_schema_audit.rs`; baseline behavioural
  scenario asserts missing login credential storage.
- Users persistence:
  Covered through `users` table (`id`, `display_name`).
  Evidence: audit rules in `user_state_schema_audit.rs`; baseline behavioural
  scenario asserts users coverage.
- Profile persistence:
  Covered for the current minimal profile model (maps to
  `users.display_name`).
  Evidence: audit rules treat profile as covered when users storage is covered.
- Interests persistence:
  Dual model detected (`user_preferences.interest_theme_ids` plus
  `user_interest_themes`), which is ambiguous for persistence ownership.
  Evidence: baseline behavioural scenario asserts `DualModel` coverage.

## Migration decisions

- Profile storage migration: `NotRequired`
  Rationale: current profile payload can be satisfied by the existing `users`
  table shape (`id`, `display_name`).
- Interests storage migration: `Required`
  Rationale: dual storage models are present, so canonical interests
  persistence must be selected before 3.5.3 and 3.5.4 work.
- Interests revision tracking migration: `NotRequired`
  Rationale: baseline schema exposes `revision` on `user_preferences`, so
  revision tracking capability is available even while storage remains
  dual-model.
- Interests stale-write conflict migration: `NotRequired`
  Rationale: conflict handling derives from revision tracking capability, which
  is currently present on the baseline schema.

## Downstream implications for roadmap 3.5.x

- 3.5.2 can proceed independently for `LoginService` and `UsersQuery` DB-backed
  wiring, but login credential persistence remains a tracked gap.
- 3.5.3 and 3.5.4 must choose a canonical interests persistence model before
  final adapter and conflict-contract implementation.
- 3.5.5 and 3.5.6 should assert both DB-backed and fixture-fallback startup
  modes while preserving the migration decisions captured here.

## Verification evidence

The following suites validate audit behaviour:

- Unit: `cargo test -p backend user_state_schema_audit --lib`
- Behavioural: `cargo test -p backend --test user_state_schema_audit_bdd`

These tests cover happy, unhappy, and edge paths:

- happy: baseline schema audit completes and reports expected coverage
- unhappy: missing `users` table requires users/profile migrations
- edge: canonical interests model with revision can avoid interests migrations
