# Implement catalogue explore and descriptors HTTP endpoints (roadmap 3.2.3)

This ExecPlan is a living document. The sections `Constraints`, `Tolerances`,
`Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`, and
`Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: IN PROGRESS

## Purpose / Big Picture

The PWA needs two session-authenticated read endpoints that return pre-assembled
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
- Tests must use `rstest` fixtures and `rstest-bdd` for behavioural scenarios.
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

- [x] (2026-02-15) Write ExecPlan to `docs/execplans/backend-3-2-3-catalogue-endpoints.md`.
- [ ] Stage A: Add `generated_at` to `DescriptorSnapshot` and update all consumers.
- [ ] Stage B: Create catalogue HTTP handler module with response DTOs and error mapping.
- [ ] Stage C: Wire handlers into HttpState, server, and OpenAPI.
- [ ] Stage D: Create recording test doubles for catalogue/descriptor ports.
- [ ] Stage E: Write BDD feature files and step implementations.
- [ ] Stage F: Write unit tests for handler response mapping.
- [ ] Stage G: Fix lint warnings, run all gates, record architecture decision.
- [ ] Stage H: Mark roadmap 3.2.3 as done.

## Surprises & Discoveries

(None yet.)

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

(To be filled on completion.)

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
