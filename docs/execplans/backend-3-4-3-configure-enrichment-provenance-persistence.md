# Configure enrichment provenance persistence and admin reporting endpoints (roadmap 3.4.3)

This Execution Plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

This plan covers roadmap item 3.4.3 only:
`Configure enrichment provenance persistence (source URL, timestamp, and
bounding box) and expose it via admin reporting endpoints.`

## Purpose / big picture

Roadmap 3.4.2 delivered the Overpass enrichment worker, quota policy, circuit
breaker logic, and enrichment metrics. The remaining gap is auditability and
operational visibility for enrichment provenance.

After this work, each successful enrichment run will persist provenance with:

- source URL;
- import timestamp;
- bounding box.

Admin reporting endpoints will expose that persisted provenance through domain
ports and inbound adapters, keeping persistence details inside outbound
adapters.

Observable success criteria:

- successful enrichment jobs write provenance rows with URL, timestamp, and
  bounds;
- admin reporting endpoint returns provenance rows in deterministic order;
- unit tests (`rstest`) and behavioural tests (`rstest-bdd`) cover happy,
  unhappy, and edge paths;
- behavioural suites use existing `pg-embedded-setup-unpriv` cluster patterns;
- architecture docs record the 3.4.3 decision;
- roadmap item 3.4.3 is marked done only after all required gates pass.

## Constraints

- Scope is roadmap 3.4.3 only. Do not redesign 3.4.2 quota, semaphore, or
  circuit policies.
- Preserve dependency direction:
  - `domain` defines contracts and owns orchestration;
  - `outbound` implements persistence adapters;
  - `inbound` consumes domain ports through `HttpState`.
- Domain modules must not import Actix, Diesel, SQL, or outbound adapters.
- Inbound handlers must not call Diesel or SQL directly.
- `server/state_builders.rs` remains the composition root for concrete wiring.
- Migrations must be additive, reversible, and include explicit bounds checks.
- Keep reporting endpoint contract bounded (`limit` defaults/max) to avoid
  unbounded reads.
- Keep docs in en-GB-oxendict and wrap prose to project standards.

## Tolerances (exception triggers)

- If delivering `/api/v1/admin/...` requires a new RBAC model beyond current
  session auth, stop and escalate.
- If provenance writes require replacing the existing POI persistence contract
  with a broader transactional port, stop and confirm scope expansion.
- If historical backfill is required for old enrichment rows, stop and split a
  dedicated backfill task.
- If file churn exceeds 30 files or about 2,200 net LOC, split into staged
  milestones.
- If `make check-fmt`, `make lint`, or `make test` fail after three
  consecutive fix attempts, stop with retained logs.

## Risks

- Risk: POI upsert succeeds while provenance persistence fails.
  Mitigation: define one explicit policy, test it, and document it in the
  architecture decision.

- Risk: source URL stored in provenance drifts from actual adapter endpoint.
  Mitigation: persist URL from the exact source invocation context.

- Risk: admin route naming implies stronger auth controls than currently
  implemented.
  Mitigation: document current auth semantics and keep scope focused on
  reporting contract.

- Risk: reporting query performance degrades as rows grow.
  Mitigation: index by import timestamp and enforce bounded pagination.

- Risk: coordinate validation diverges between domain and database checks.
  Mitigation: reuse one validation rule path and mirror it in DB constraints.

## Agent team

Use this ownership split during implementation. Ownership is by concern, not by
exclusive file lock.

- `Reimu Hakurei` (architecture seam owner):
  domain ports/entities, worker seam changes, dependency guardrails.
- `Mike Haggar` (test strategy owner):
  `rstest` and `rstest-bdd` happy/unhappy/edge matrix, fixture/world updates,
  embedded Postgres behaviour.
- `Marisa Kirisame` (delivery closure owner):
  sequencing, tolerances, gate evidence commands, architecture doc decision
  capture, roadmap closure criteria.

Coordination rules:

1. Baseline and design before adapter/HTTP implementation.
2. Land domain and persistence seams before endpoint wiring.
3. Run targeted tests at each milestone before progressing.
4. Run full required gates only at integrated final state.

## Context and orientation

Primary roadmap and architecture references:

- `docs/backend-roadmap.md` (item 3.4.3 scope).
- `docs/wildside-backend-architecture.md` (hexagonal boundaries and 3.4.2
  decision record).

Testing and quality references:

- `docs/rust-testing-with-rstest-fixtures.md`.
- `docs/rstest-bdd-users-guide.md`.
- `docs/pg-embed-setup-unpriv-users-guide.md`.
- `docs/rust-doctest-dry-guide.md`.
- `docs/complexity-antipatterns-and-refactoring-strategies.md`.

Current code anchors:

- `backend/src/domain/overpass_enrichment_worker/mod.rs`
- `backend/src/domain/overpass_enrichment_worker/runtime.rs`
- `backend/src/domain/ports/overpass_enrichment_source.rs`
- `backend/src/inbound/http/state.rs`
- `backend/src/server/state_builders.rs`
- `backend/src/server/mod.rs`
- `backend/src/outbound/persistence/schema.rs`
- `backend/src/outbound/persistence/mod.rs`
- `backend/tests/features/overpass_enrichment.feature`
- `backend/tests/overpass_enrichment_bdd.rs`
- `backend/tests/overpass_enrichment_bdd/world.rs`
- `backend/tests/support/cluster_skip.rs`
- `backend/tests/support/atexit_cleanup.rs`

## Milestones

## Milestone 0 - Baseline and seam confirmation

Confirm current behaviour before any edits.

Tasks:

- verify overpass worker success path currently persists POIs/metrics only;
- verify no existing admin enrichment reporting endpoint wiring;
- verify ingestion provenance seam is ingestion-specific and not reused by
  enrichment.

Validation:

```bash
set -o pipefail
make test | tee /tmp/test-$(get-project)-$(git branch --show)-baseline.out
```

## Milestone 1 - Domain contracts and worker seam

Add dedicated domain contracts for enrichment provenance persistence and
reporting.

Planned contract shape:

- `EnrichmentProvenanceRecord`:
  - `source_url`;
  - `imported_at`;
  - `bounding_box`.
- `ListEnrichmentProvenanceRequest` with bounded `limit` and optional cursor.
- `ListEnrichmentProvenanceResponse` with ordered records and optional next
  cursor.
- `EnrichmentProvenanceRepository`:
  - `persist(record)`;
  - `list_recent(request)`.

Worker seam:

- extend `OverpassEnrichmentWorkerPorts` with provenance repository;
- update success path to persist provenance on successful source fetch;
- map persistence failures through existing worker error envelope.

Done when:

- new port/types compile and are exported through `domain::ports::mod.rs`;
- worker retains existing quota/circuit behaviour and tests remain green;
- new unit tests cover provenance success/failure mapping.

## Milestone 2 - Migration and Diesel outbound adapter

Implement persistent storage for enrichment provenance.

Migration shape:

- new migration:
  `backend/migrations/<timestamp>_create_overpass_enrichment_provenance/`;
- table: `overpass_enrichment_provenance`;
- required fields:
  - `source_url TEXT`;
  - `imported_at TIMESTAMPTZ`;
  - bounds (`min_lng`, `min_lat`, `max_lng`, `max_lat`);
  - `created_at`.
- constraints:
  - coordinate range checks;
  - `min <= max` checks.
- index:
  - `idx_overpass_enrichment_provenance_imported_at` on `(imported_at DESC)`.

Adapter shape:

- new outbound adapter:
  `backend/src/outbound/persistence/diesel_enrichment_provenance_repository.rs`;
- update `schema.rs` and `outbound/persistence/mod.rs` exports.

Done when:

- migration up/down apply cleanly;
- adapter inserts and lists records in deterministic order;
- integration tests prove URL/time/bounds round-trip fidelity.

## Milestone 3 - Admin reporting endpoint and state wiring

Add reporting endpoint through inbound HTTP adapter and domain query port.

Proposed endpoint contract:

- route: `GET /api/v1/admin/enrichment/provenance`;
- query:
  - `limit` (default 50, max 200);
  - `before` (optional RFC3339 cursor);
- response:
  - `records: [{ sourceUrl, importedAt, boundingBox }]`;
  - optional `nextBefore`.

Wiring steps:

- add handler module (for example `backend/src/inbound/http/admin_enrichment.rs`);
- extend `HttpState` / `HttpStatePorts` for provenance query port;
- wire DB-backed adapter in `build_http_state` when pool exists;
- wire fixture/no-op fallback when pool is absent;
- register route in `build_app`;
- update OpenAPI surface in `backend/src/doc.rs`.

Done when:

- route is live under `/api/v1` and uses `SessionContext` + `HttpState`;
- no inbound module imports outbound adapter concrete types;
- OpenAPI captures endpoint and response schema.

## Milestone 4 - Test matrix (unit + behavioural)

Implement required coverage with explicit happy/unhappy/edge paths.

Unit tests (`rstest`):

- overpass worker provenance write path:
  - happy: provenance persisted with expected URL/time/bounds;
  - unhappy: provenance persistence connection/query failure mapping;
  - edge: provenance persisted when source returns zero POIs.
- provenance query domain service/repository mapping:
  - happy: newest-first ordering;
  - unhappy: repository errors mapped to domain errors;
  - edge: empty set and limit boundary handling.
- HTTP handler tests:
  - happy: authenticated request returns expected payload shape;
  - unhappy: unauthenticated request returns `401`;
  - unhappy: repository unavailable returns `503`;
  - edge: invalid query returns `400`.

Behavioural tests (`rstest-bdd`):

- extend `backend/tests/features/overpass_enrichment.feature` with provenance
  persistence scenarios (happy/unhappy/edge);
- add dedicated admin reporting feature:
  - happy: persisted provenance is visible via endpoint;
  - unhappy: persistence/query unavailable path;
  - unhappy: auth required path;
  - edge: empty data and filtered cursor path.

`pg-embedded-setup-unpriv` strategy:

- reuse `shared_cluster_handle()` and template database provisioning patterns;
- preserve `SKIP-TEST-CLUSTER` handling semantics from existing suites;
- keep world fixtures holding runtime + `TemporaryDatabase` handles for cleanup.

Done when:

- new unit tests and BDD scenarios pass consistently;
- existing 3.4.2 scenarios remain green.

## Milestone 5 - Documentation and roadmap closure

Update documentation to record decisions and close roadmap state.

Docs updates:

- `docs/wildside-backend-architecture.md`:
  - add driven port entry for enrichment provenance reporting/persistence;
  - add `3.4.3 Implementation Decision (YYYY-MM-DD)` near 3.4.2 section;
  - record endpoint contract and persistence policy.
- `docs/backend-roadmap.md`:
  - mark item 3.4.3 done only after all gates and tests pass.

Done when:

- architecture decision and roadmap status match implemented behaviour.

## Milestone 6 - Final gates and evidence

Run required gates with retained logs:

```bash
set -o pipefail
make check-fmt 2>&1 | tee /tmp/check-fmt-$(get-project)-$(git branch --show).out
make lint 2>&1 | tee /tmp/lint-$(get-project)-$(git branch --show).out
make test 2>&1 | tee /tmp/test-$(get-project)-$(git branch --show).out
```

Branch-specific expected evidence paths:

- `/tmp/check-fmt-wildside-backend-3-4-3-configure-enrichment-provenance-persistence.out`
- `/tmp/lint-wildside-backend-3-4-3-configure-enrichment-provenance-persistence.out`
- `/tmp/test-wildside-backend-3-4-3-configure-enrichment-provenance-persistence.out`

## Progress

- [x] (2026-02-28) Confirmed branch context and roadmap 3.4.3 scope.
- [x] (2026-02-28) Loaded execplan + hexagonal architecture guidance.
- [x] (2026-02-28) Created context pack `pk_rqbwrf2z` with code/doc anchors.
- [x] (2026-02-28) Ran agent-team planning synthesis for architecture, testing,
      and closure.
- [x] (2026-02-28) Drafted this implementation ExecPlan artifact.
- [ ] Implement Milestone 1.
- [ ] Implement Milestone 2.
- [ ] Implement Milestone 3.
- [ ] Implement Milestone 4.
- [ ] Implement Milestone 5.
- [ ] Run Milestone 6 and capture evidence.

## Surprises & Discoveries

- Existing overpass worker path persists POIs and metrics only; provenance
  persistence is currently absent.
- Existing ingestion provenance schema/port (`osm_ingestion_provenance`) is
  specific to 3.4.1 rerun semantics and should not be reused directly for 3.4.3.
- Current `HttpState` and app route wiring expose no admin enrichment reporting
  seam.

## Decision Log

- Decision: add a dedicated enrichment provenance domain port and entity rather
  than extending ingestion provenance types.
  Rationale: ingestion and enrichment lifecycles and contracts differ.
  Date/Author: 2026-02-28 / Codex.

- Decision: keep admin reporting endpoint in inbound HTTP, consuming a domain
  query port via `HttpState`.
  Rationale: maintains hexagonal boundaries and existing route wiring patterns.
  Date/Author: 2026-02-28 / Codex.

- Decision: treat auth model changes as out of scope for 3.4.3.
  Rationale: roadmap requires reporting exposure, not RBAC redesign.
  Date/Author: 2026-02-28 / Codex.

- Decision: record final persistence policy (transactional vs sequential write
  handling) in architecture docs when implementation is complete.
  Rationale: this is a key operational behaviour and must be explicit.
  Date/Author: 2026-02-28 / Codex.

## Outcomes & Retrospective

Pending implementation.

Closure notes to capture when complete:

- final endpoint contract and pagination limits shipped;
- final persistence failure policy shipped;
- architecture decision section path and date;
- roadmap checkbox update confirmation;
- full gate evidence paths and statuses;
- any deferred follow-up work.
