# Phase 2.3.2: PWA Preferences and Annotations Endpoints

This Execution Plan (ExecPlan) is a living document. The sections `Progress`,
`Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective` must
be kept up to date as work proceeds.

## Purpose / Big Picture

Implement the HTTP endpoints required for PWA preferences and annotations:

- `GET/PUT /api/v1/users/me/preferences`
- `GET /api/v1/routes/{route_id}/annotations`
- `POST /api/v1/routes/{route_id}/notes`
- `PUT /api/v1/routes/{route_id}/progress`

All endpoints must go through the inbound HTTP adapter and call domain
services (driving ports). Idempotent mutations use the existing
`Idempotency-Key` contract and the shared `IdempotencyRepository`. Error
responses must reuse the existing domain `Error` envelope so clients always see
consistent payloads.

This is **step 2.3.2** from the backend roadmap. Contract tests for
deterministic retries are tracked in 2.3.3.

Success is observable when:

- Endpoints are implemented in `backend/src/inbound/http` using domain ports.
- Idempotency keys are honoured for preferences, notes, and progress mutations.
- Error envelopes follow the existing `domain::Error` response mapping.
- OpenAPI schema definitions cover new request/response types.
- Unit tests (`rstest`) and behavioural tests (`rstest-bdd` v0.3.2) cover happy
  and unhappy paths, including edge cases.
- Postgres-backed integration tests use `pg-embedded-setup-unpriv`.
- `docs/wildside-backend-architecture.md` records new design decisions.
- `docs/backend-roadmap.md` marks 2.3.2 as done.
- `make check-fmt`, `make lint`, and `make test` succeed.

## Progress

- [x] (2026-01-03 03:10Z) Draft ExecPlan for PWA preferences and annotations
  endpoints.
- [x] (2026-01-03 03:12Z) Attempt to use Code Graph MCP to map inbound
  handlers, ports, and adapters touched by this change.
- [x] (2026-01-03 03:13Z) Confirm API payload shapes from
  `docs/wildside-pwa-data-model.md` and
  `docs/wildside-backend-architecture.md`.
- [x] (2026-01-03 03:16Z) Add driving query ports for read endpoints and
  service implementations for preferences and annotations with idempotency
  support.
- [x] (2026-01-03 03:17Z) Implement HTTP handlers and route wiring for the new
  endpoints.
- [x] (2026-01-03 03:18Z) Add OpenAPI schema wrappers and documentation paths.
- [x] (2026-01-03 03:19Z) Add unit and behavioural tests, plus Postgres-backed
  integration tests.
- [x] (2026-01-03 03:20Z) Update architecture documentation and roadmap.
- [x] (2026-01-03 03:25Z) Run quality gates (`make check-fmt`, `make lint`,
  `make test`, and `make markdownlint`).

## Surprises & Discoveries

- Observation: Code Graph MCP resources were not available in this
  environment.
  Evidence: MCP resource discovery returned no resources.
- Observation: Embedded PostgreSQL downloads initially hit GitHub rate limits
  during `make test`, and TFLint plugin initialization failed until plugins
  were downloaded into a workspace directory.
  Evidence: Re-running `make test` with `PG_WORKER_PATH` and `make lint` with
  `TFLINT_PLUGIN_DIR` completed successfully.

## Decision Log

- Decision: Use inbound request/response DTOs with `utoipa::ToSchema` for
  preferences and annotations instead of adding schema wrappers for the
  domain types.
  Rationale: Keeps the domain layer framework-agnostic while documenting HTTP
  payloads.
  Date/Author: 2026-01-03 03:14Z / Codex
- Decision: Map foreign key violations for route annotations using the
  database constraint name when available.
  Rationale: Ensures missing routes surface as `RouteNotFound` even when
  PostgreSQL error messages omit table names.
  Date/Author: 2026-01-03 03:23Z / Codex.

## Outcomes & Retrospective

Endpoints for PWA preferences and route annotations are implemented with
idempotency handling, documented in OpenAPI, and backed by domain services and
ports. Unit, behavioural, and Postgres-backed integration tests pass after
resolving embedded Postgres and TFLint plugin setup issues. The main lesson
learned is to preconfigure workspace-scoped paths for external tooling in
sandboxed environments to avoid transient failures.

## Context and Orientation

Key locations (repository-relative):

- `backend/src/inbound/http/`: HTTP adapter modules and handlers.
- `backend/src/inbound/http/routes.rs`: idempotency header parsing pattern.
- `backend/src/inbound/http/error.rs`: error envelope mapping.
- `backend/src/inbound/http/state.rs`: HTTP port wiring.
- `backend/src/domain/ports/`: driving and driven port traits.
- `backend/src/domain/route_submission/`: idempotency orchestration pattern.
- `backend/src/outbound/persistence/`: Diesel repositories.
- `backend/tests/adapter_guardrails/`: HTTP adapter harness and doubles.
- `backend/tests/support/pg_embed.rs`: pg-embedded bootstrap helper.
- `docs/wildside-pwa-data-model.md`: PWA payload shapes and expectations.
- `docs/wildside-backend-architecture.md`: API and idempotency contracts.
- `docs/backend-roadmap.md`: task tracking.

Terminology:

- *Driving port*: domain service invoked by inbound adapters.
- *Driven port*: repository interface implemented by outbound adapters.
- *Idempotency*: safe retries keyed by `Idempotency-Key` header with payload
  fingerprinting.

## Plan of Work

### 1. Map current surface area (Code Graph MCP + repo scan)

- Use the Code Graph MCP to locate inbound handlers, state wiring, and
  idempotency helpers (if MCP is available).
- Confirm where new endpoints should register (`backend/src/server/mod.rs`,
  `backend/src/doc.rs`) and how existing tests configure the HTTP harness.

### 2. Lock API contracts and payload shapes

- Align request/response shapes with the PWA model:
  - Preferences update includes `interestThemeIds`, `safetyToggleIds`,
    `unitSystem`, and optional `expectedRevision`.
  - Note upsert includes `noteId`, `poiId?`, `body`, and optional
    `expectedRevision`.
  - Progress update includes `visitedStopIds` and optional
    `expectedRevision`.
  - Annotation fetch returns `notes` and `progress` (and decide if `routeId`
    is included).
- Record any deviations or clarifications in
  `docs/wildside-backend-architecture.md`.

### 3. Add driving query ports for reads

- Introduce `UserPreferencesQuery` and `RouteAnnotationsQuery` ports (mirroring
  `UserProfileQuery` and `UsersQuery`) to avoid inbound adapters touching
  repositories directly.
- Provide fixture implementations and mocks for tests.

### 4. Implement preference and annotation services

- Add domain services implementing `UserPreferencesCommand` and
  `RouteAnnotationsCommand`, following the idempotency patterns in
  `RouteSubmissionServiceImpl`.
- Ensure idempotency uses `MutationType::{Preferences, Notes, Progress}` and
  serializes responses for replay.
- Map repository errors (`RevisionMismatch`, `RouteNotFound`, connection and
  query errors) to appropriate `domain::Error` variants.
- Add unit tests covering idempotency hits, conflicts, revision mismatches,
  and repository error mapping with `rstest` and mocks.

### 5. Implement inbound HTTP handlers

- Add handler modules (new `preferences.rs` / `annotations.rs` or extend
  `users.rs` and `routes.rs`, keeping files under 400 lines).
- Parse and validate:
  - `route_id` path parameters as UUID.
  - `Idempotency-Key` header via a shared helper (reuse or move the
    `routes.rs` extraction logic).
  - Request JSON payloads with explicit error mapping to
    `Error::invalid_request` and detail payloads.
- Call domain command/query ports and return JSON responses that use camelCase
  keys and the standard error envelope.

### 6. Wire ports into HTTP state and server setup

- Extend `HttpStatePorts`/`HttpState` with the new query/command ports.
- Update `backend/src/server/mod.rs` to register the new endpoints.
- Update adapter harnesses (`backend/tests/adapter_guardrails`) to include
  recording doubles for the new ports.

### 7. OpenAPI schema updates

- Add schema wrappers to `backend/src/inbound/http/schemas.rs` for:
  `UserPreferences`, `RouteNote`, `RouteProgress`, and the annotations response
  envelope.
- Register paths and schemas in `backend/src/doc.rs`.
- Extend BDD OpenAPI tests to assert the new schemas are referenced.

### 8. Testing strategy (unit + behavioural + Postgres)

- Unit tests (`rstest`):
  - Validation helpers for UUID parsing, unit system parsing, and request
    mapping.
  - Service-layer idempotency and revision behaviour with mocks.
- Behavioural tests (`rstest-bdd`):
  - Session-required access, happy-path responses, and error envelopes.
  - Unhappy paths: invalid idempotency key, invalid revision, conflict
    responses, and missing routes.
- Integration tests with Postgres:
  - Exercise Diesel repositories and service logic using
    `pg-embedded-setup-unpriv` and `backend/tests/support/pg_embed.rs`.

### 9. Documentation and roadmap updates

- Update `docs/wildside-backend-architecture.md` with new decisions and
  endpoint payload definitions.
- Mark 2.3.2 as done in `docs/backend-roadmap.md`.

### 10. Quality gates

- Run `make fmt`, `make check-fmt`, `make lint`, `make test`.
- For documentation changes, also run `make markdownlint` and `make nixie`
  (when Mermaid diagrams change).

## Concrete Steps

Run these commands from the repository root:

1. Format code (after changes):

   ```bash
   set -o pipefail
   timeout 300 make fmt 2>&1 | tee /tmp/wildside-fmt.log
   ```

2. Lint Markdown (after doc changes):

   ```bash
   set -o pipefail
   timeout 300 make markdownlint 2>&1 | tee /tmp/wildside-markdownlint.log
   ```

3. Validate Mermaid (only if diagrams change):

   ```bash
   set -o pipefail
   timeout 300 make nixie 2>&1 | tee /tmp/wildside-nixie.log
   ```

4. Check formatting:

   ```bash
   set -o pipefail
   timeout 300 make check-fmt 2>&1 | tee /tmp/wildside-check-fmt.log
   ```

5. Lint:

   ```bash
   set -o pipefail
   timeout 300 make lint 2>&1 | tee /tmp/wildside-lint.log
   ```

6. Test:

   ```bash
   set -o pipefail
   timeout 600 make test 2>&1 | tee /tmp/wildside-test.log
   ```

Notes:

- If the embedded Postgres helper needs a writable worker path, run:
  `PG_WORKER_PATH=/tmp/pg_worker make test`.

## Validation and Acceptance

Acceptance criteria:

1. **Endpoints implemented**: All four endpoints exist with session
   enforcement, correct payloads, and idempotency handling.
2. **Ports respected**: HTTP handlers use driving ports; repositories are not
   accessed directly from inbound adapters.
3. **Error envelope consistency**: Invalid input and conflict responses use
   the existing `domain::Error` JSON shape and trace ID header.
4. **OpenAPI updated**: New schemas and paths appear in `ApiDoc`.
5. **Testing complete**: Unit, behavioural, and Postgres-backed tests pass via
   `pg-embedded-setup-unpriv`.
6. **Documentation updated**: Architecture decisions logged and roadmap entry
   marked done.

## Idempotence and Recovery

- Idempotent mutations use `IdempotencyRepository` for key reservation and
  response replay.
- Conflicting payloads return `409 Conflict`; matching payloads replay the
  stored response.
- If a command fails, fix the issue and re-run only the failed command, then
  repeat the relevant quality gate(s).

## Files to Create/Modify

### New files (likely)

- `backend/src/domain/ports/user_preferences_query.rs`
- `backend/src/domain/ports/route_annotations_query.rs`
- `backend/src/domain/preferences/service.rs` (or similar)
- `backend/src/domain/annotations/service.rs` (or similar)
- `backend/src/inbound/http/preferences.rs` (if split for size)
- `backend/src/inbound/http/annotations.rs` (if split for size)
- `backend/tests/features/user_preferences.feature`
- `backend/tests/features/route_annotations.feature`
- `backend/tests/user_preferences_bdd.rs`
- `backend/tests/route_annotations_bdd.rs`

### Modify

- `backend/src/inbound/http/state.rs`
- `backend/src/inbound/http/mod.rs`
- `backend/src/inbound/http/routes.rs` (idempotency helper reuse)
- `backend/src/inbound/http/users.rs` (if preferences live here)
- `backend/src/server/mod.rs`
- `backend/src/inbound/http/schemas.rs`
- `backend/src/doc.rs`
- `backend/tests/adapter_guardrails/doubles.rs`
- `backend/tests/adapter_guardrails/harness.rs`
- `docs/wildside-backend-architecture.md`
- `docs/backend-roadmap.md`
