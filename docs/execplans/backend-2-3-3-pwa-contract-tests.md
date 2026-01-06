# PWA Contract Tests for Optimistic Concurrency, Idempotency, and Determinism

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

This document must be maintained in accordance with the execplans skill
guidelines. See `docs/documentation-style-guide.md` for formatting rules.

## Purpose / Big Picture

Task 2.3.3 of the backend roadmap requires contract tests that validate the
behavioural guarantees of the PWA endpoints: optimistic concurrency via revision
fields, idempotency conflict detection, and deterministic response replay for
retried requests. These tests exercise the HTTP adapter layer end-to-end,
ensuring clients can safely implement offline-first patterns with reliable retry
semantics.

After this work is complete, developers and CI will have behavioural test
coverage demonstrating:

1. **Optimistic concurrency**: Updates with stale `expectedRevision` values
   return HTTP 409 with structured conflict details (`expectedRevision` vs
   `actualRevision`).
2. **Idempotency conflicts**: Reusing an `Idempotency-Key` header with a
   different request payload returns HTTP 409.
3. **Deterministic replay**: Retrying an identical request with the same
   `Idempotency-Key` returns the cached response with `replayed: true` and
   identical payload structure.

Observable outcome: Running `make test` executes the new scenarios, and the
test output includes scenarios named for each contract guarantee.

## Constraints

Hard invariants that must hold throughout implementation:

- **Hexagonal boundaries**: Tests must exercise the inbound HTTP adapter via
  the test harness (`actix-web::test`), not by calling domain services directly.
  Contract tests validate the HTTP contract, not internal wiring.
- **No production code changes**: This task adds tests only. The endpoint
  implementations from task 2.3.2 are already complete.
- **Test isolation**: Each scenario must be independent. BDD scenarios using
  the adapter guardrail harness receive fresh test doubles per scenario.
- **Repository contract tests**: Tests requiring real PostgreSQL behaviour
  (e.g., concurrent writes) must use `pg-embedded-setup-unpriv` and handle
  cluster unavailability gracefully via the existing skip pattern.
- **File size limit**: No single test file may exceed 400 lines. Split by
  feature area if necessary.
- **rstest-bdd v0.3.2**: Use the project's pinned version of `rstest-bdd` for
  behavioural tests.

## Tolerances (Exception Triggers)

Thresholds that trigger escalation when breached:

- **Scope**: If implementation requires changes to more than 8 files or 600
  lines of code (net), stop and escalate.
- **Interface**: If any public API signature must change to enable testing,
  stop and escalate.
- **Dependencies**: If a new external dependency is required beyond what is
  already in `Cargo.toml`, stop and escalate.
- **Iterations**: If tests still fail after 3 attempts to fix, stop and
  escalate.
- **Ambiguity**: If multiple valid interpretations exist for a contract
  guarantee, document options and escalate.

## Risks

Known uncertainties that might affect the plan:

- **Risk**: Embedded PostgreSQL cluster may not be available in all CI
  environments.
  - Severity: low
  - Likelihood: low
  - Mitigation: Use the existing `handle_cluster_setup_failure` pattern to skip
    tests gracefully when the cluster is unavailable. Tests print
    `SKIP-TEST-CLUSTER:` messages for visibility.

- **Risk**: Simulating concurrent revision conflicts in tests requires careful
  setup to avoid flaky behaviour.
  - Severity: medium
  - Likelihood: medium
  - Mitigation: For BDD tests using test doubles, configure the double to
    return a conflict error directly. For repository contract tests, use
    sequential operations with explicit revision values rather than true
    concurrency.

- **Risk**: Idempotency replay tests may be sensitive to JSON serialization
  order.
  - Severity: low
  - Likelihood: low
  - Mitigation: Compare semantic equality of response bodies rather than raw
    string equality. Use `serde_json::Value` comparisons.

## Progress

- [x] (2026-01-05) Stage A: Analyse existing test coverage and identify gaps.
- [x] (2026-01-05) Stage B: Add BDD scenarios for optimistic concurrency conflicts.
- [x] (2026-01-05) Stage C: Add BDD scenarios for idempotency conflicts.
- [x] (2026-01-05) Stage D: Add BDD scenarios for deterministic replay.
- [x] (2026-01-05) Stage E: Review repository-level contract tests (already complete).
- [x] (2026-01-05) Stage F: Validate with `make check-fmt`, `make lint`, `make test`.
- [x] (2026-01-05) Stage G: Update roadmap and architecture documentation.

## Surprises & Discoveries

- Observation: Existing repository tests already had good coverage of revision
  semantics.
  Evidence: The `diesel_user_preferences_repository.rs` and
  `diesel_route_annotation_repository.rs` files included round-trip, revision
  update, and revision mismatch tests.
  Impact: Stage E became a review rather than implementation. No additional
  repository tests were needed.

## Decision Log

- **Decision**: Focus on BDD scenarios using test doubles for HTTP-layer
  contract tests, supplemented by repository-level tests for persistence
  semantics.
  - Rationale: BDD scenarios with test doubles are fast and deterministic,
    while repository tests with embedded PostgreSQL validate the SQL-level
    revision checking. This layered approach provides comprehensive coverage
    without redundancy.
  - Date/Author: 2026-01-05 / Assistant

- **Decision**: Group tests by contract guarantee (concurrency, conflicts,
  replay) rather than by endpoint.
  - Rationale: The contract guarantees apply uniformly across preferences,
    notes, and progress endpoints. Grouping by guarantee makes the test suite
    easier to navigate and ensures consistent coverage.
  - Date/Author: 2026-01-05 / Assistant

## Outcomes & Retrospective

### Outcomes

The plan delivered 9 new BDD scenarios across 2 feature files:

**pwa_preferences.feature** (3 new scenarios):

- `Preferences update rejects stale revision` - optimistic concurrency
- `Preferences update rejects idempotency conflict` - idempotency detection
- `Preferences update replays cached response` - deterministic replay

**pwa_annotations.feature** (6 new scenarios):

- `Progress update surfaces conflicts` - updated to verify revision details
- `Note upsert rejects stale revision` - optimistic concurrency
- `Note upsert rejects idempotency conflict` - idempotency detection
- `Note upsert replays cached response` - deterministic replay
- `Progress update rejects idempotency conflict` - idempotency detection
- `Progress update replays cached response` - deterministic replay

All 376 tests pass. Quality gates (`check-fmt`, `lint`, `test`) succeed.

### Retrospective

- **What worked well**: The existing test infrastructure (harness, doubles,
  bdd_common helpers) made adding new scenarios straightforward. The layered
  test approach (BDD + repository) was validated by finding that repository
  tests were already comprehensive.

- **What could be improved**: Stages B, C, and D were implemented together
  since they shared step definitions. Future plans could combine related stages
  when implementation is straightforward.

- **Lessons learned**: The test double pattern with configurable responses is
  highly effective for contract testing HTTP semantics without coupling to
  implementation details.

## Context and Orientation

The PWA endpoints were implemented in task 2.3.2 and provide:

- `GET/PUT /api/v1/users/me/preferences` for user preferences
- `GET /api/v1/routes/{route_id}/annotations` for notes and progress
- `POST /api/v1/routes/{route_id}/notes` for note upsert
- `PUT /api/v1/routes/{route_id}/progress` for progress update

All mutation endpoints support:

1. **Optimistic concurrency** via `expectedRevision` in the request body. When
   the database revision does not match, the repository returns
   `RevisionMismatch { expected, actual }`, which the service maps to HTTP 409
   with structured details.

2. **Idempotency** via the `Idempotency-Key` HTTP header (UUID). The
   `IdempotencyRepository` stores `(key, user_id, mutation_type, payload_hash)`
   with a 24-hour TTL. Matching payload = replay; different payload = conflict.

### Key files

- **HTTP handlers**: `backend/src/inbound/http/preferences.rs`,
  `backend/src/inbound/http/annotations.rs`
- **Domain services**: `backend/src/domain/preferences_service.rs`,
  `backend/src/domain/annotations/service.rs`
- **Domain ports**: `backend/src/domain/ports/user_preferences_command.rs`,
  `backend/src/domain/ports/route_annotations_command.rs`
- **Repository ports**: `backend/src/domain/ports/user_preferences_repository.rs`,
  `backend/src/domain/ports/route_annotation_repository.rs`
- **Diesel adapters**: `backend/src/outbound/persistence/diesel_user_preferences_repository.rs`,
  `backend/src/outbound/persistence/diesel_route_annotation_repository.rs`
- **Existing BDD tests**: `backend/tests/pwa_preferences_bdd.rs`,
  `backend/tests/pwa_annotations_bdd.rs`
- **Existing features**: `backend/tests/features/pwa_preferences.feature`,
  `backend/tests/features/pwa_annotations.feature`
- **Test harness**: `backend/tests/adapter_guardrails/harness.rs`,
  `backend/tests/support/bdd_common.rs`
- **Test doubles**: `backend/tests/adapter_guardrails/doubles_preferences.rs`,
  `backend/tests/adapter_guardrails/doubles_annotations.rs`

### Existing test coverage

The existing BDD tests cover:

- Authenticated fetch of preferences and annotations
- Validation errors (invalid unit system)
- Idempotency key capture (verifying the key reaches the domain)
- Single conflict scenario for progress update

**Gaps identified**:

- No scenarios for preferences revision mismatch
- No scenarios for note revision mismatch
- No scenarios for idempotency key conflict (different payload)
- No scenarios for deterministic replay (same key, same payload)
- Repository tests exist but do not cover idempotency semantics

## Plan of Work

### Stage A: Analyse existing test coverage

Read existing feature files and test implementations to confirm the gap
analysis above. Document any additional gaps discovered.

**Go/no-go**: Proceed if gaps match expectations; escalate if existing coverage
is more complete than anticipated.

### Stage B: Add BDD scenarios for optimistic concurrency conflicts

Add scenarios to the existing feature files for revision mismatch handling:

1. **Preferences revision mismatch**: Client sends `expectedRevision: 1` but
   database has revision 2. Expect HTTP 409 with conflict details.

2. **Note revision mismatch**: Client sends `expectedRevision: 1` for note
   update but database has revision 2. Expect HTTP 409.

3. **Progress revision mismatch**: Extend existing conflict scenario to verify
   the response body contains `expectedRevision` and `actualRevision` fields.

Implementation approach:

- Add scenarios to `backend/tests/features/pwa_preferences.feature` and
  `backend/tests/features/pwa_annotations.feature`.
- Add step definitions to the corresponding `*_bdd.rs` files.
- Configure test doubles to return `RevisionMismatch` errors.

**Go/no-go**: Scenarios pass with `cargo test`.

### Stage C: Add BDD scenarios for idempotency conflicts

Add scenarios for idempotency key conflict detection:

1. **Preferences idempotency conflict**: First request succeeds, second request
   with same key but different payload returns HTTP 409.

2. **Note idempotency conflict**: Same pattern for note upsert.

3. **Progress idempotency conflict**: Same pattern for progress update.

Implementation approach:

- Configure test doubles to return `IdempotencyConflict` on second call.
- Verify HTTP 409 response with appropriate error code.

**Go/no-go**: Scenarios pass with `cargo test`.

### Stage D: Add BDD scenarios for deterministic replay

Add scenarios for idempotent replay semantics:

1. **Preferences replay**: Request with same key and payload returns cached
   response with `replayed: true`.

2. **Note replay**: Same pattern for note upsert.

3. **Progress replay**: Same pattern for progress update.

Implementation approach:

- Configure test doubles to return a successful response with `replayed: true`.
- Verify response body matches expected structure and includes `replayed` field.

**Go/no-go**: Scenarios pass with `cargo test`.

### Stage E: Add repository-level contract tests for revision semantics

Extend existing repository tests in `backend/tests/diesel_user_preferences_repository.rs`
and `backend/tests/diesel_route_annotation_repository.rs` to cover additional
edge cases:

1. **First save without revision**: Verify initial save with
   `expected_revision: None` creates revision 1.

2. **Update with correct revision**: Verify update with matching revision
   succeeds and increments revision.

3. **Update with stale revision**: Verify update with non-matching revision
   returns `RevisionMismatch` with both expected and actual values.

4. **Concurrent-like scenario**: Save revision 1, attempt update expecting
   revision 1 but actually at revision 2 (simulated by prior update).

These tests already partially exist; extend to ensure complete coverage of the
revision semantics.

**Go/no-go**: Tests pass with `cargo test`.

### Stage F: Validate with quality gates

Run the full quality gate suite:

    make check-fmt && make lint && make test 2>&1 | tee /tmp/pwa-contract-tests.log

Verify all tests pass. Fix any failures.

**Go/no-go**: All gates pass.

### Stage G: Update roadmap and architecture documentation

1. Mark task 2.3.3 as complete in `docs/backend-roadmap.md`.
2. Add a design decision entry to `docs/wildside-backend-architecture.md`
   documenting the contract test coverage for PWA endpoints.

**Go/no-go**: Documentation updated, `make markdownlint` passes.

## Concrete Steps

All commands run from the repository root.

### Stage A

1. Read existing feature files:

       cat backend/tests/features/pwa_preferences.feature
       cat backend/tests/features/pwa_annotations.feature

2. Read existing test implementations:

       cat backend/tests/pwa_preferences_bdd.rs
       cat backend/tests/pwa_annotations_bdd.rs

3. Confirm gap analysis matches expectations.

### Stage B

1. Edit `backend/tests/features/pwa_preferences.feature` to add:

       Scenario: Preferences update rejects stale revision
         Given a running server with session middleware
         And the client has an authenticated session
         And the preferences command returns a revision mismatch
         When the client updates preferences with expected revision 1
         Then the response is a conflict error with revision details

2. Edit `backend/tests/features/pwa_annotations.feature` to add:

       Scenario: Note upsert rejects stale revision
         Given a running server with session middleware
         And the client has an authenticated session
         And the note command returns a revision mismatch
         When the client upserts a note with expected revision 1
         Then the response is a conflict error with revision details

       Scenario: Progress update includes revision details in conflict
         Given a running server with session middleware
         And the client has an authenticated session
         And the progress update is configured to conflict with revision details
         When the client updates progress with expected revision 1
         Then the response is a conflict error with revision details

3. Add corresponding step definitions to `backend/tests/pwa_preferences_bdd.rs`
   and `backend/tests/pwa_annotations_bdd.rs`.

4. Run tests:

       cargo test --test pwa_preferences_bdd --test pwa_annotations_bdd 2>&1 | tee /tmp/stage-b.log

### Stage C

1. Edit feature files to add idempotency conflict scenarios:

       Scenario: Preferences update rejects idempotency conflict
         Given a running server with session middleware
         And the client has an authenticated session
         And the preferences command returns an idempotency conflict
         When the client updates preferences with a reused idempotency key
         Then the response is a conflict error with idempotency details

2. Add similar scenarios for notes and progress.

3. Add step definitions.

4. Run tests:

       cargo test --test pwa_preferences_bdd --test pwa_annotations_bdd 2>&1 | tee /tmp/stage-c.log

### Stage D

1. Edit feature files to add replay scenarios:

       Scenario: Preferences update replays cached response
         Given a running server with session middleware
         And the client has an authenticated session
         And the preferences command returns a replayed response
         When the client updates preferences with a reused idempotency key
         Then the response is ok
         And the preferences response includes replayed true

2. Add similar scenarios for notes and progress.

3. Add step definitions.

4. Run tests:

       cargo test --test pwa_preferences_bdd --test pwa_annotations_bdd 2>&1 | tee /tmp/stage-d.log

### Stage E

1. Review existing repository tests:

       cat backend/tests/diesel_user_preferences_repository.rs
       cat backend/tests/diesel_route_annotation_repository.rs

2. Add any missing edge cases for revision semantics.

3. Run repository tests:

       cargo test diesel_user_preferences_repository diesel_route_annotation_repository 2>&1 | tee /tmp/stage-e.log

### Stage F

1. Run quality gates:

       make check-fmt 2>&1 | tee /tmp/check-fmt.log
       make lint 2>&1 | tee /tmp/lint.log
       make test 2>&1 | tee /tmp/test.log

2. Fix any failures and re-run.

### Stage G

1. Edit `docs/backend-roadmap.md`:

   Change `- [ ] 2.3.3.` to `- [x] 2.3.3.`

2. Edit `docs/wildside-backend-architecture.md` to add design decision:

       - **2026-01-XX:** Add contract tests for PWA endpoint guarantees
         (optimistic concurrency, idempotency conflicts, deterministic replay).
         BDD scenarios exercise the HTTP adapter layer with test doubles;
         repository tests validate SQL-level revision semantics with embedded
         PostgreSQL. Coverage ensures clients can safely implement offline-first
         retry patterns.

3. Run markdown lint:

       make markdownlint

## Validation and Acceptance

Quality criteria (what "done" means):

- **Tests**: All existing tests pass, plus new scenarios:
  - 3 optimistic concurrency scenarios (preferences, notes, progress)
  - 3 idempotency conflict scenarios
  - 3 deterministic replay scenarios
  - Repository edge cases as needed
- **Lint/typecheck**: `make lint` passes with no warnings.
- **Formatting**: `make check-fmt` passes.
- **Documentation**: Roadmap updated, architecture decision recorded.

Quality method (how we check):

    make check-fmt && make lint && make test

Expected output: All commands exit 0. Test output includes scenario names
matching the new coverage areas.

## Idempotence and Recovery

All steps are safe to repeat:

- Feature file edits are additive; re-running creates no duplicates.
- Step definitions are idempotent (functions with unique names).
- Test runs are isolated; no persistent state modified.
- Documentation edits can be repeated without side effects.

If a step fails partway through, re-run from the beginning of that stage.

## Artifacts and Notes

Expected test output (partial):

    running X tests
    test pwa_preferences::pwa_preferences ... ok
    test pwa_annotations::pwa_annotations ... ok
    ...
    test result: ok. X passed; 0 failed

## Interfaces and Dependencies

No new interfaces or dependencies required. Uses existing:

- `rstest` (0.18.x) for unit test fixtures
- `rstest-bdd` (0.3.2) for behavioural scenarios
- `actix-web::test` for HTTP integration testing
- `pg-embedded-setup-unpriv` for embedded PostgreSQL
- Existing test harness and doubles infrastructure
