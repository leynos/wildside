# Implement offline bundle and walk session HTTP endpoints (roadmap 3.3.3)

This Execution Plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

There is no `PLANS.md` in this repository, so this ExecPlan is the primary
execution reference for roadmap item 3.3.3.

Implementation started after explicit approval and this plan now captures the
completed delivery.

## Purpose / big picture

Roadmap item 3.3.3 requires production HTTP delivery for offline manifests and
walk completion writes:

- `GET /api/v1/offline/bundles`
- `POST /api/v1/offline/bundles`
- `DELETE /api/v1/offline/bundles/{bundle_id}`
- `POST /api/v1/walk-sessions`

After this work, the Progressive Web App (PWA) can sync offline bundle
manifests and submit walk
sessions through the same session-authenticated API boundary as existing
preferences and annotation flows. Persistence must remain confined to outbound
Diesel adapters behind domain ports. Endpoints must return stable identifiers
and surface revision semantics only where those semantics exist in the domain.

Observable success criteria:

- All four endpoints are routed and documented in OpenAPI.
- `POST` responses preserve and return stable client-provided UUID identifiers.
- Offline bundle mutations support idempotency via `Idempotency-Key`.
- Repository calls happen through domain ports and driving services, not direct
  Diesel access in handlers.
- Unit tests (`rstest`) and behavioural tests (`rstest-bdd`) cover happy,
  unhappy, and edge paths for all endpoints.
- `docs/wildside-backend-architecture.md` records the 3.3.3 design decisions.
- `docs/backend-roadmap.md` marks 3.3.3 done only after gates pass.
- `make check-fmt`, `make lint`, and `make test` pass with captured logs.

## Constraints

- Scope is roadmap task 3.3.3 only. Do not start 3.4.x or 3.5.x work.
- Preserve hexagonal boundaries:
  - domain defines use-cases/ports and invariants;
  - inbound HTTP maps request/response and delegates to ports;
  - outbound adapters own SQL and serialization details.
- Keep `backend/src/domain` free from Actix and Diesel imports.
- Keep handlers thin and consistent with existing inbound adapter patterns
  (`preferences`, `annotations`, `catalogue`, `routes`).
- Use existing domain entities (`OfflineBundle`, `WalkSession`) and existing
  repository ports (`OfflineBundleRepository`, `WalkSessionRepository`) as the
  persistence boundary.
- Implement any new orchestration behind driving ports exposed from
  `backend/src/domain/ports`.
- Reuse the idempotency repository contract for offline bundle mutations
  (`MutationType::Bundles`).
- Use session auth (`SessionContext`) for all new endpoints.
- Tests must use `rstest` and `rstest-bdd` and include happy, unhappy, and
  edge cases.
- Behavioural integration requiring PostgreSQL must use the existing
  `pg-embedded-setup-unpriv` harness patterns.
- Keep Markdown wrapped at 80 columns (except headings/tables/code blocks).

## Tolerances (exception triggers)

- Scope tolerance: if implementation requires changing public contracts outside
  these four endpoints, stop and record options before proceeding.
- Interface tolerance: if existing domain entities require new persisted fields
  (for example adding a revision field), stop and obtain approval.
- Churn tolerance: if diff exceeds 20 files or 1,600 net LOC, split into
  explicit sub-milestones and re-approve.
- Dependency tolerance: if any new external crate is needed, stop and
  escalate.
- Validation tolerance: if `make check-fmt`, `make lint`, or `make test` fails
  more than three consecutive fix attempts, stop with logs and analysis.
- Runtime tolerance: if embedded PostgreSQL tests are repeatedly flaky under
  default nextest parallelism, run with `NEXTEST_TEST_THREADS=1`, record why,
  and re-run full `make test`.

## Risks

- Risk: endpoint Data Transfer Object (DTO) shape diverges from
  `docs/wildside-pwa-data-model.md`, causing client compatibility drift.
  Severity: high.
  Likelihood: medium.
  Mitigation: define explicit request/response schema mapping and Behaviour-Driven
  Development (BDD) checks.

- Risk: HTTP handlers call repositories directly without driving-port
  orchestration, leaking idempotency/retry rules into adapters.
  Severity: high.
  Likelihood: medium.
  Mitigation: add explicit driving command/query ports for offline and walk
  flows and wire those into `HttpState`.

- Risk: offline mutation idempotency collisions across users or payloads.
  Severity: medium.
  Likelihood: medium.
  Mitigation: reuse existing `IdempotencyRepository` lookup/store pattern with
  `MutationType::Bundles`, plus conflict/replay tests.

- Risk: ambiguity around “revision updates where applicable”.
  Severity: medium.
  Likelihood: medium.
  Mitigation: keep revision semantics unchanged from current domain model:
  no revision field for offline bundles/walk sessions; return stable IDs and
  timestamps (`updatedAt`) and note this decision in architecture docs.

- Risk: adding new HTTP endpoints breaks OpenAPI registration completeness.
  Severity: medium.
  Likelihood: medium.
  Mitigation: update `backend/src/doc.rs` path/components and assert coverage
  in existing OpenAPI BDD suite.

## Agent team

Implementation will use a focused agent team for both design and coding tasks.

- Agent A: domain port and service design.
  Owns:
  - `backend/src/domain/ports/*offline*`
  - `backend/src/domain/ports/*walk*`
  - new domain services for offline/walk orchestration
  - `backend/src/domain/mod.rs` exports

- Agent B: inbound HTTP and server wiring.
  Owns:
  - `backend/src/inbound/http/*` for new handlers and DTOs
  - `backend/src/inbound/http/state.rs`
  - `backend/src/server/state_builders.rs`
  - `backend/src/server/mod.rs`
  - `backend/src/doc.rs`

- Agent C: behavioural/unit tests and documentation.
  Owns:
  - handler unit tests and domain service tests
  - `backend/tests/*` and `backend/tests/features/*` Behaviour-Driven
    Development (BDD) artefacts
  - `docs/wildside-backend-architecture.md`
  - `docs/backend-roadmap.md` checkbox update (only at completion)

Coordination rules:

- Each agent edits owned files only and ignores unrelated concurrent edits.
- Merge order: A -> B -> C.
- Re-run relevant tests at each merge boundary.
- Final full gates run once after all merges, before roadmap checkbox update.

## Progress

- [x] (2026-02-21 20:45Z) Confirmed branch context and in-scope instructions.
- [x] (2026-02-21 20:45Z) Loaded required skills:
      `execplans`, `hexagonal-architecture`, `leta`, and `grepai`.
- [x] (2026-02-21 20:45Z) Collected roadmap and architecture constraints for
      3.3.3 plus related 3.3.1/3.3.2 decisions.
- [x] (2026-02-21 20:45Z) Used explorer-agent team to gather:
      roadmap acceptance details, boundary constraints, and testing strategy.
- [x] (2026-02-21 20:45Z) Drafted this ExecPlan at
      `docs/execplans/backend-3-3-3-bundle-and-walk-endpoints.md`.
- [x] (2026-02-22) Implemented domain driving ports/services for offline
      bundle and walk session endpoint orchestration.
- [x] (2026-02-22) Implemented inbound HTTP handlers + DTOs + route wiring for
      the four endpoints.
- [x] (2026-02-22) Wired services into HTTP state builders with DB-backed and
      fixture fallbacks.
- [x] (2026-02-22) Registered OpenAPI paths/schemas and verified documentation
      builds with the existing server/documentation tests.
- [x] (2026-02-22) Added/updated `rstest` unit tests for parser/mapping/service
      logic.
- [x] (2026-02-22) Added/updated `rstest-bdd` behavioural suites for endpoint
      behaviour.
- [x] (2026-02-22) Recorded architecture design decisions for 3.3.3 in
      `docs/wildside-backend-architecture.md`.
- [x] (2026-02-22) Marked roadmap item 3.3.3 done in
      `docs/backend-roadmap.md`.
- [x] (2026-02-22) Ran required gates and captured logs:
      `make check-fmt`, `make lint`, `make test`.
- [x] (2026-02-22) Committed only after all gates passed.

## Surprises & Discoveries

- Observation (2026-02-21): repository adapters and schema for offline bundles
  and walk sessions already exist and are tested at repository contract level.
  Impact: 3.3.3 can focus on driving-port orchestration + HTTP delivery and
  avoid persistence redesign.

- Observation (2026-02-21): architecture doc already references
  `OfflineBundleCommand` and `WalkSessionCommand`, but code currently exposes
  only repository ports and no corresponding driving command ports.
  Impact: 3.3.3 should add driving ports/services to align implementation with
  documented architecture.

- Observation (2026-02-21): current `HttpState` and server route wiring have no
  offline/walk entries.
  Impact: state wiring and harness doubles must be extended before endpoint BDD
  tests can pass.

## Decision Log

- Decision: implement explicit driving ports/services for offline bundle and
  walk session HTTP operations instead of having handlers call repository ports
  directly.
  Rationale: aligns with architecture section “Driving ports” and keeps
  idempotency/retry policy out of handlers.
  Date/Author: 2026-02-21 / Codex.

- Decision: treat “revision updates where applicable” as “preserve and return
  existing mutable version markers in domain payloads” and not introduce a new
  revision field for offline/walk entities in 3.3.3.
  Rationale: current domain types use IDs and timestamps, not numeric revision
  fields; adding new revision columns would exceed roadmap scope.
  Date/Author: 2026-02-21 / Codex.

- Decision: keep offline bundle idempotency on `POST` and `DELETE` only, using
  existing `MutationType::Bundles`.
  Rationale: matches architecture and existing idempotency taxonomy.
  Date/Author: 2026-02-21 / Codex.

## Context and orientation

Primary references:

- `docs/backend-roadmap.md` (phase 3.3.3 scope).
- `docs/wildside-backend-architecture.md`:
  REST table, driven/driving ports, and 3.3.1/3.3.2 decisions.
- `docs/wildside-pwa-data-model.md` (offline and walk payload contracts).
- `docs/rust-testing-with-rstest-fixtures.md`.
- `docs/rstest-bdd-users-guide.md`.
- `docs/pg-embed-setup-unpriv-users-guide.md`.
- `docs/rust-doctest-dry-guide.md`.
- `docs/complexity-antipatterns-and-refactoring-strategies.md`.

Implementation anchors in current codebase:

- Domain entities:
  - `backend/src/domain/offline/bundle.rs`
  - `backend/src/domain/walks/session.rs`
- Existing repository ports:
  - `backend/src/domain/ports/offline_bundle_repository.rs`
  - `backend/src/domain/ports/walk_session_repository.rs`
- Existing outbound adapters:
  - `backend/src/outbound/persistence/diesel_offline_bundle_repository.rs`
  - `backend/src/outbound/persistence/diesel_walk_session_repository.rs`
- Existing inbound wiring patterns:
  - `backend/src/inbound/http/preferences.rs`
  - `backend/src/inbound/http/annotations.rs`
  - `backend/src/inbound/http/state.rs`
  - `backend/src/server/state_builders.rs`
  - `backend/src/server/mod.rs`
- Existing BDD harness for session-authenticated endpoint testing:
  - `backend/tests/adapter_guardrails/doubles.rs`
  - `backend/tests/adapter_guardrails/harness.rs`
  - `backend/tests/support/pwa_http.rs`

## Milestones

### Milestone 1: Domain driving ports and service contracts

Add explicit driving ports in `backend/src/domain/ports`:

- `OfflineBundleCommand` for create/update and delete mutations.
- `OfflineBundleQuery` for list/read operations.
- `WalkSessionCommand` for session creation and completion summary projection.

Define request/response DTOs in domain-port modules using domain-native types
and optional idempotency key where applicable. Export all new ports from
`backend/src/domain/ports/mod.rs`.

Create implementation services in domain layer (new modules) that compose:

- `OfflineBundleRepository` + `IdempotencyRepository` for bundle mutations.
- `WalkSessionRepository` for walk writes and summary derivation.

Service responsibilities:

- payload hashing and idempotency lookup/store for bundle mutations;
- stable identifier preservation (`id` unchanged);
- deterministic error mapping (`conflict`, `invalid_request`,
  `service_unavailable`, `internal_error`);
- no infrastructure types exposed across the boundary.

### Milestone 2: HTTP DTOs and handlers

Add handler module(s) under `backend/src/inbound/http`:

- `offline.rs` for bundle list/create/delete endpoints.
- `walk_sessions.rs` for walk session creation endpoint.

Add request/response DTOs that map camelCase API payloads to domain requests.
Use explicit parsers similar to preferences/annotations modules.

Proposed endpoint behaviour:

- `GET /api/v1/offline/bundles`:
  - requires session;
  - supports device scoping via request payload/query (final shape chosen in
    implementation and documented);
  - returns `200` list.
- `POST /api/v1/offline/bundles`:
  - requires session;
  - optional `Idempotency-Key`;
  - returns bundle payload including stable `id` and updated timestamp.
- `DELETE /api/v1/offline/bundles/{bundle_id}`:
  - requires session;
  - optional `Idempotency-Key`;
  - returns delete status for requested stable `bundle_id`.
- `POST /api/v1/walk-sessions`:
  - requires session;
  - validates domain draft;
  - returns created session/completion payload with stable `id`.

### Milestone 3: State wiring and routing

Update dependency wiring so handlers consume only domain ports:

- `backend/src/inbound/http/state.rs`:
  add offline/walk command/query fields.
- `backend/src/server/state_builders.rs`:
  build DB-backed services when pool exists and fixture fallbacks otherwise.
- `backend/src/server/mod.rs`:
  mount services under `/api/v1` scope.
- `backend/src/inbound/http/mod.rs`:
  export new modules.

### Milestone 4: OpenAPI and API documentation integration

Register new endpoints and any new schemas in:

- `backend/src/doc.rs`
- potentially `backend/src/inbound/http/schemas.rs` for additional schema
  wrappers.

Ensure operation IDs, tags, and error schema usage match existing conventions.

### Milestone 5: Unit test coverage (`rstest`)

Add/extend unit tests for:

- request parser validation and error details;
- idempotency replay/conflict logic in new domain services;
- DTO mapping and stable-ID roundtripping;
- session/auth failure mapping in handler-level tests where appropriate.

Use `#[rstest]` fixtures and table-driven cases for invalid payload variants.

### Milestone 6: Behavioural test coverage (`rstest-bdd`)

Add/extend BDD feature coverage for the new endpoints, likely via:

- new feature file under `backend/tests/features/`, and
- new scenario runner under `backend/tests/`.

Reuse shared PWA harness and add doubles for offline/walk ports.

Required scenario categories:

- happy paths:
  - list bundles;
  - create bundle;
  - delete bundle;
  - create walk session.
- unhappy paths:
  - unauthenticated access;
  - invalid UUID/body fields;
  - idempotency conflict;
  - repository connection/query failures.
- edge paths:
  - idempotency replay on bundle mutation;
  - anonymous vs owner-scoped bundle listing;
  - walk payload with duplicate stats/POIs rejected by domain validation.

For integration that needs PostgreSQL behaviour, continue using
`pg-embedded-setup-unpriv` setup patterns already present in the test suite.

### Milestone 7: Documentation and roadmap updates

Record architecture decisions in `docs/wildside-backend-architecture.md`:

- final endpoint payload contract decisions;
- idempotency behaviour for bundle mutations;
- interpretation of revision semantics for 3.3.3.

Only after code and gates succeed, update roadmap checkbox:

- `docs/backend-roadmap.md` mark 3.3.3 as done.

### Milestone 8: Gate execution and evidence

Run gates with `tee` logs and `pipefail`:

    set -o pipefail && make check-fmt | tee /tmp/check-fmt-$(get-project)-$(git branch --show).out
    set -o pipefail && make lint | tee /tmp/lint-$(get-project)-$(git branch --show).out
    set -o pipefail && make test | tee /tmp/test-$(get-project)-$(git branch --show).out

If embedded PostgreSQL flakiness appears:

    set -o pipefail && NEXTEST_TEST_THREADS=1 make test | tee /tmp/test-threads1-$(get-project)-$(git branch --show).out

Do not mark roadmap done and do not commit until all required gates pass.

## Outcomes & Retrospective

Completed outcomes:

- Added driving ports and domain services for offline bundle mutations/queries
  and walk session creation, keeping repository details in outbound adapters.
- Delivered the four roadmap endpoints:
  `GET/POST/DELETE /api/v1/offline/bundles` and
  `POST /api/v1/walk-sessions`.
- Preserved stable identifiers in API responses (`id` values are caller-stable
  UUIDs), and kept revision semantics unchanged because these entities do not
  use numeric revision counters.
- Extended OpenAPI registration and endpoint tagging for the new HTTP surface.
- Added `rstest` and `rstest-bdd` coverage for happy paths, validation/auth
  failures, and idempotency/edge handling.
- Recorded architecture decisions and marked roadmap item 3.3.3 done.

Retrospective notes:

- Existing repository contracts and migrations from 3.3.1/3.3.2 kept endpoint
  work focused on orchestration and inbound mapping, which reduced delivery
  risk.
- The main complexity point was idempotent mutation orchestration in the domain
  service; extracting helper structures avoided clippy complexity drift and
  preserved readability.
