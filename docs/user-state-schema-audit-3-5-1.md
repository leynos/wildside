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

Primary implementation references:

- `backend/src/domain/user_state_schema_audit.rs`
- `backend/tests/user_state_schema_audit_bdd.rs`
- `backend/tests/features/user_state_schema_audit.feature`

## Coverage findings

| Area | Coverage finding | Evidence |
| --- | --- | --- |
| Login persistence | Missing credential storage in schema (`MissingCredentialStorage`). | Audit rules in `user_state_schema_audit.rs`; baseline behavioural scenario asserts missing login credential storage. |
| Users persistence | Covered through `users` table (`id`, `display_name`). | Audit rules in `user_state_schema_audit.rs`; baseline behavioural scenario asserts users coverage. |
| Profile persistence | Covered for the current minimal profile model (maps to `users.display_name`). | Audit rules treat profile as covered when users storage is covered. |
| Interests persistence | Dual model detected (`user_preferences.interest_theme_ids` plus `user_interest_themes`), which is ambiguous for persistence ownership. | Baseline behavioural scenario asserts `DualModel` coverage. |

## Migration decisions

| Decision area | Result | Rationale |
| --- | --- | --- |
| Profile storage migration | `NotRequired` | Current profile payload can be satisfied by the existing `users` table shape (`id`, `display_name`). |
| Interests storage migration | `Required` | Dual storage models are present, so canonical interests persistence must be selected before 3.5.3 and 3.5.4 work. |
| Interests revision tracking migration | `Required` | Revision semantics are not canonical for the dedicated interests path while dual storage remains. |
| Interests stale-write conflict handling migration | `Required` | Conflict handling depends on canonical revision tracking and remains unresolved for the interests-specific path. |

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
