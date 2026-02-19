# Implement catalogue explore and descriptors HTTP endpoints (roadmap 3.2.3)

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

## Purpose / Big Picture

The Progressive Web App (PWA) needs two session-authenticated read endpoints
that return pre-assembled
snapshots of catalogue and descriptor data. After this change, an authenticated
client can:

1. `GET /api/v1/catalogue/explore` -- receive the full explore catalogue
   (categories, route summaries, themes, collections, trending highlights, and
   the community pick) with a `generated_at` timestamp and HTTP cache headers.
2. `GET /api/v1/catalogue/descriptors` -- receive all descriptor registries
   (tags, badges, safety toggles, safety presets, interest themes) with a
   `generated_at` timestamp and HTTP cache headers.

Both endpoints return `401` when no session is present, `503` when the
repository is unavailable, and `500` for unexpected failures. Responses include
a `Cache-Control` header and a top-level `generatedAt` field so clients can
detect staleness.

Completing this task marks roadmap item 3.2.3 as done and unblocks phase 3.3
(offline bundles) and phase 4 (pagination infrastructure).

## Constraints

- Hexagonal architecture: handlers must be thin inbound adapters that call
  domain ports. No Diesel, no raw SQL, no outbound imports in handler code.
- Domain types must not derive utoipa `ToSchema`. OpenAPI schemas live in the
  inbound adapter layer (`backend/src/inbound/http/schemas.rs`).
- All domain types remain framework-agnostic (no actix, no utoipa derives).
- Existing public interfaces of `CatalogueRepository`, `DescriptorRepository`,
  and all domain types must remain stable except for the addition of
  `generated_at` to `DescriptorSnapshot`.
- `make check-fmt`, `make lint`, and `make test` must all pass.
- Tests must use `rstest` fixtures and `rstest-bdd` for
  behaviour-driven development (BDD) scenarios.
- No new external crate dependencies.

## Tolerances (Exception Triggers)

- Scope: if changes touch more than 25 files (net new + modified), escalate.
- Interface: the only port signature change allowed is adding `generated_at` to
  `DescriptorSnapshot`. Any other port change requires escalation.
- Iterations: if tests still fail after 5 attempts at a fix, escalate.
- Ambiguity: if cache header policy needs negotiation (e.g. max-age duration),
  escalate with options.

## Risks

- Risk: Adding `generated_at` to `DescriptorSnapshot` breaks existing tests
  that construct snapshots without the field.
  Severity: low
  Likelihood: high
  Mitigation: Update `FixtureDescriptorRepository`, all test constructors,
  and the Diesel adapter in a single atomic commit. The field already exists
  on `ExploreCatalogueSnapshot` so the pattern is established.

- Risk: The adapter guardrails harness (`AdapterWorld`) must be extended with
  catalogue/descriptor recording doubles, which increases harness complexity.
  Severity: medium
  Likelihood: high
  Mitigation: Follow the established `recording_double!` macro pattern used
  by existing doubles. Keep the new doubles minimal (single method each).

- Risk: Existing `cluster_skip` warnings (unused imports/functions) may cause
  lint failures.
  Severity: low
  Likelihood: high
  Mitigation: Fix the existing warnings as part of the cleanup commit.

## Progress

- [x] (2026-02-15) Write ExecPlan to
  `docs/execplans/backend-3-2-3-catalogue-endpoints.md`.
- [x] (2026-02-15) Stage A: Add `generated_at` to
  `DescriptorSnapshot` and update all consumers.
- [x] (2026-02-15) Stage B: Create catalogue HTTP handler
  module with response DTOs and error mapping.
- [x] (2026-02-15) Stage C: Wire handlers into HttpState,
  server, and OpenAPI.
- [x] (2026-02-15) Stage D: Create recording test doubles
  for catalogue/descriptor ports.
- [x] (2026-02-15) Stage E: Write BDD feature files and
  step implementations.
- [x] (2026-02-15) Stage F: Unit tests written inline in
  Stage B (catalogue.rs `#[cfg(test)]`).
- [x] (2026-02-15) Stage G: Fix lint warnings, run all
  gates, record architecture decision.
- [x] (2026-02-15) Stage H: Mark roadmap 3.2.3 as done.

## Surprises & Discoveries

- Observation: `clippy::expect_used` is denied in non-test code via `lib.rs`.
  Evidence: Clippy rejected `.expect()` calls in `From` impls for response DTOs.
  Impact: Changed `From<Snapshot>` to `TryFrom<Snapshot>` with `Error = domain::Error`,
  using a `to_json_value` helper that maps `serde_json` errors to `Error::internal`.
  Handlers use `?` to propagate, tests use `.unwrap()`.

- Observation: `module-max-lines` custom lint (whitaker) limits modules to
  400 lines.
  Evidence: Adding catalogue doubles wiring pushed `harness.rs` to 413
  lines.
  Impact: Inlined `HttpWsStateInputs` struct into direct `HttpStatePorts`
  construction in the `world()` function, reducing the file to 381 lines.

- Observation: Catalogue BDD test fixtures used `.expect()` for embedded
  PostgreSQL setup, causing 180-second Continuous Integration (CI) timeouts
  when the cluster is
  unavailable.
  Evidence: CI failure in `catalogue_descriptor_ingestion_bdd` with
  `postgresql_embedded::setup() failed: operation timed out after 180.0s`.
  Impact: Converted `setup_test_context()` to `Result<TestContext, String>`
  and adopted the `handle_cluster_setup_failure` pattern in both
  `catalogue_descriptor_ingestion_bdd` and
  `catalogue_descriptor_read_models_bdd`. Tests now fail fast with a clear
  diagnostic instead of hanging.

## Decision Log

- Decision: Endpoints require session authentication.
  Rationale: User explicitly confirmed session-required over public access.
  Date/Author: 2026-02-15 / user confirmation.

- Decision: Use `private, no-cache, must-revalidate` for Cache-Control.
  Rationale: Data is session-scoped (authenticated), so `private` is
  appropriate. `no-cache, must-revalidate` forces revalidation on each
  request, matching the pattern used by `get_preferences`. The snapshot
  `generatedAt` field gives clients a staleness signal without needing ETags
  in this iteration.
  Date/Author: 2026-02-15 / plan author.

- Decision: Create a single `catalogue.rs` handler module (not two separate
  modules) since both endpoints share the same domain area and response
  patterns.
  Rationale: Keeps the module tree flat and consistent with the existing
  pattern where `preferences.rs` contains both GET and PUT handlers.
  Date/Author: 2026-02-15 / plan author.

- Decision: Add `From<CatalogueRepositoryError>` and
  `From<DescriptorRepositoryError>` impls for `domain::Error` rather than
  creating thin driving port traits.
  Rationale: These are simple read-through operations with no business logic.
  A service layer would add unnecessary indirection. The `From` impls map
  `Connection` to `Error::service_unavailable(...)` and `Query` to
  `Error::internal(...)`, co-located with the source error types.
  Date/Author: 2026-02-15 / plan author.

## Outcomes & Retrospective

All stages completed successfully. The two new endpoints are implemented,
tested with 6 BDD scenarios (happy path, auth enforcement, error surfacing),
and documented in the architecture document.

Key outcomes:

- `GET /api/v1/catalogue/explore` and
  `GET /api/v1/catalogue/descriptors` serve pre-assembled
  snapshots behind session authentication.
- `Cache-Control: private, no-cache, must-revalidate` and
  `generatedAt` metadata enable client-side staleness
  detection.
- Response DTOs use `serde_json::Value` wrapper fields to
  keep `ToSchema` derives out of the domain layer.
- `TryFrom` conversion pattern avoids `expect()` in
  non-test code.
- Harness refactoring (inlining `HttpWsStateInputs`) keeps
  the shared test harness under the 400-line module limit
  despite growing port count.

Lessons:

- When wrapping domain types for OpenAPI,
  `serde_json::Value` is a pragmatic escape hatch that
  avoids propagating utoipa derives into the domain.
- The `recording_double!` macro smoothly handles new ports;
  the main growth pressure is in the harness's
  `AdapterWorld` struct and `world()` fixture.

## Context and Orientation

The Wildside backend is a hexagonal modular monolith built with Actix-Web and
Diesel. The codebase is structured as:

    backend/src/domain/          -- Business logic, domain types, validation
    backend/src/domain/ports/    -- Port trait definitions (hexagonal boundary)
    backend/src/inbound/http/    -- HTTP handlers (thin adapters)
    backend/src/outbound/persistence/ -- Diesel-backed adapters
    backend/src/server/          -- Server construction and wiring

Key files for this task:

- `backend/src/domain/ports/catalogue_repository.rs` -- `CatalogueRepository`
  trait and `ExploreCatalogueSnapshot` (has `generated_at: DateTime<Utc>`).
- `backend/src/domain/ports/descriptor_repository.rs` -- `DescriptorRepository`
  trait and `DescriptorSnapshot` (currently missing `generated_at`).
- `backend/src/domain/ports/mod.rs` -- Re-exports all ports and mocks.
- `backend/src/inbound/http/mod.rs` -- Declares handler modules.
- `backend/src/inbound/http/state.rs` -- `HttpState` / `HttpStatePorts` structs
  bundling Arc-wrapped port trait objects.
- `backend/src/inbound/http/schemas.rs` -- OpenAPI schema wrappers.
- `backend/src/inbound/http/preferences.rs` -- Reference handler pattern
  (session-auth, cache headers, response DTO, utoipa annotations).
- `backend/src/inbound/http/error.rs` -- `ApiResult<T>` alias and
  `ResponseError` impl mapping `domain::Error` to HTTP status codes.
- `backend/src/server/mod.rs` -- `create_server` and `build_app` wiring
  endpoints and ports.
- `backend/src/doc.rs` -- OpenAPI `ApiDoc` registration.
- `backend/tests/adapter_guardrails/harness.rs` -- Test server harness with
  `AdapterWorld`, `WorldFixture`, recording doubles.
- `backend/tests/adapter_guardrails/doubles.rs` -- Double module re-exports.
- `backend/tests/adapter_guardrails/recording_double_macro.rs` -- Macro for
  generating recording test doubles.
- `backend/tests/support/bdd_common.rs` -- Shared BDD step helpers.
- `backend/tests/pwa_annotations_bdd.rs` -- Reference BDD test file.
- `backend/tests/features/pwa_annotations.feature` -- Reference feature file.
- `backend/src/outbound/persistence/diesel_descriptor_repository.rs` -- Diesel
  descriptor adapter (needs `generated_at` addition).
- `docs/backend-roadmap.md` -- Roadmap with item 3.2.3.
- `docs/wildside-backend-architecture.md` -- Architecture document.

## Plan of Work

### Stage A: Add `generated_at` to `DescriptorSnapshot`

Add `pub generated_at: DateTime<Utc>` to `DescriptorSnapshot`, update the
fixture, Diesel adapter, and any test consumers.

### Stage B: Create catalogue HTTP handler module

Create `backend/src/inbound/http/catalogue.rs` with response DTOs,
`From<Snapshot>` impls, handler functions, and `From<PortError> for Error`
impls in the port files.

### Stage C: Wire handlers into HttpState, server, and OpenAPI

Add `catalogue` and `descriptors` ports to `HttpState`/`HttpStatePorts`,
register endpoints in `build_app`, wire Diesel repos in `create_server`,
and register in OpenAPI `ApiDoc`.

### Stage D: Create recording test doubles

Create `RecordingCatalogueRepository` and `RecordingDescriptorRepository`
using the `recording_double!` macro and extend the adapter guardrails harness.

### Stage E: Write BDD feature files and step implementations

Create `catalogue_endpoints.feature` with scenarios for happy path, auth
enforcement, and error handling. Implement step definitions.

### Stage F: Write unit tests for handler response mapping

Unit tests for `From<Snapshot>` DTO conversions using `rstest`.

### Stage G: Fix lint warnings, run all gates, record architecture decision

Fix `cluster_skip` warnings, run `make check-fmt && make lint && make test`,
update `docs/wildside-backend-architecture.md`.

### Stage H: Mark roadmap 3.2.3 as done

## Validation and Acceptance

Quality criteria:

- `make check-fmt` passes.
- `make lint` passes.
- `make test` passes (all existing plus new BDD scenarios and unit tests).
- Roadmap item 3.2.3 is marked `[x]`.
- Architecture document updated.

Quality method:

    make check-fmt && make lint && make test
