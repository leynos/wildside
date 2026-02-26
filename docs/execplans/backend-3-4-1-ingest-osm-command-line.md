# Ship the `ingest-osm` CLI with backend-owned ingestion controls (roadmap 3.4.1)

This Execution Plan (ExecPlan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE (implementation delivered; documentation closure updated)

There is no `PLANS.md` in this repository, so this ExecPlan is the primary
execution reference for roadmap item 3.4.1.

Implementation has completed, and this document now records delivered outcomes,
gate evidence, and residual risks.

## Purpose / big picture

Roadmap item 3.4.1 requires delivery of a Rust `ingest-osm` command-line
interface by integrating `wildside-engine` ingestion capabilities via the
`wildside-data` crate while keeping backend-owned behaviour explicit and
testable:

- launch geofence filtering;
- provenance persistence (`source_url`, input digest, timestamp, bounding box);
- deterministic reruns keyed by geofence and input digest.

After this work, backend ingestion should be repeatable, auditable, and
compatible with the existing hexagonal boundaries already established for
catalogue and descriptor ingestion repositories.

Observable success criteria:

- A Rust CLI command exists in this workspace for `ingest-osm` and is wired
  through domain ports (not direct SQL calls in CLI code).
- Geofence filtering is executed as an explicit backend concern before
  persistence writes.
- Provenance metadata is persisted for each ingest run with stable digest and
  geofence identifiers.
- Deterministic reruns reuse the same geofence+digest key and avoid duplicate
  persistence effects.
- Unit tests (`rstest`) and behavioural tests (`rstest-bdd`) cover happy,
  unhappy, and edge paths for parsing, orchestration, persistence, and reruns.
- Local PostgreSQL behavioural tests run through `pg-embedded-setup-unpriv`.
- `docs/wildside-backend-architecture.md` records design decisions taken.
- `docs/backend-roadmap.md` marks 3.4.1 done only after all gates pass.
- `make check-fmt`, `make lint`, and `make test` all pass with captured logs.

## Constraints

- Scope is roadmap item 3.4.1 only. Do not implement 3.4.2 or 3.4.3.
- Preserve hexagonal architecture invariants:
  - domain owns contracts and orchestration;
  - outbound adapters own persistence and integration details;
  - inbound CLI adapter owns argument parsing and command invocation only.
- Keep persistence details confined to outbound adapters, reusing or extending
  domain ports under `backend/src/domain/ports`.
- Do not bypass existing ingestion ports:
  - `CatalogueIngestionRepository`;
  - `DescriptorIngestionRepository`.
- Introduce any new provenance/rerun persistence through new or extended domain
  ports first, then outbound Diesel adapters.
- Keep domain modules free of Diesel, Actix, and external CLI framework types.
- Use existing project testing patterns:
  - `rstest` unit tests;
  - `rstest-bdd` behavioural tests with `.feature` files;
  - embedded PostgreSQL fixtures via `pg-embedded-setup-unpriv`.
- Keep documentation in en-GB-oxendict style and wrap Markdown paragraphs at
  80 columns.

## Tolerances (exception triggers)

- Scope tolerance: if work requires route-generation or enrichment behaviour
  outside 3.4.1, stop and split into a follow-up plan.
- Dependency tolerance: if `wildside-engine` cannot be consumed without adding
  new crates or Git dependencies not already approved, stop and present
  integration options with trade-offs.
- Schema tolerance: if provenance/rerun guarantees require changes that exceed
  a single cohesive migration set, stop and split into staged milestones.
- Churn tolerance: if the implementation exceeds 24 files or 1,800 net LOC,
  stop and re-scope into sequenced commits/milestones.
- Validation tolerance: if any required gate fails more than three consecutive
  fix attempts, stop with logs and root-cause notes.
- Runtime tolerance: if embedded PostgreSQL tests are flaky under default
  parallelism, run with `NEXTEST_TEST_THREADS=1`, document why, and still run
  the full required gate stack.

## Risks

- Risk: `wildside-engine` API shape may not map cleanly to existing backend
  ingestion entities.
  Status: mitigated.
  Mitigation applied: added `WildsideDataOsmSourceRepository` as a thin outbound
  adapter behind `OsmSourceRepository`, keeping mapping isolated from domain
  orchestration.

- Risk: provenance schema may under-specify rerun determinism, causing duplicate
  writes or non-auditable reruns.
  Status: mitigated.
  Mitigation applied: added migration
  `2026-02-24-000000_create_osm_ingestion_provenance` with unique rerun key
  constraint on `(geofence_id, input_digest)` and repository conflict mapping.

- Risk: CLI orchestration grows into a high-complexity control flow.
  Status: mitigated.
  Mitigation applied: split responsibilities into CLI parsing
  (`backend/src/bin/ingest_osm.rs`), domain orchestration
  (`backend/src/domain/osm_ingestion.rs`), and outbound adapters.

- Risk: behavioural tests may silently validate fixtures instead of migrated
  schema.
  Status: mitigated.
  Mitigation applied: BDD scenarios run with Diesel repositories on temporary
  PostgreSQL instances and include a dropped-schema failure path.

- Risk: unit-test evidence for the domain service may be incomplete when the
  service module exceeds file-length limits.
  Status: mitigated.
  Mitigation applied: extracted tests into
  `backend/src/domain/osm_ingestion_tests/` and kept
  `backend/src/domain/osm_ingestion.rs` under 400 lines while preserving
  `rstest` coverage.

## Agent team

Implementation ran through a focused agent team for design and coding:

- Agent A: domain contract design and orchestration.
  Owns:
  - `backend/src/domain/ports/*ingestion*`;
  - new domain ingestion orchestration modules;
  - typed error mapping for ingestion and deterministic reruns.

- Agent B: persistence and migrations.
  Owns:
  - migration files under `backend/migrations/*`;
  - outbound adapters in `backend/src/outbound/persistence/*`;
  - schema/model updates for provenance and rerun keys.

- Agent C: CLI inbound adapter and integration wiring.
  Owns:
  - CLI binary/module wiring in `backend/src/bin/*` and related command
    modules;
  - argument parsing, geofence selection, and invocation boundaries;
  - app/bootstrap wiring needed for command execution.

- Agent D: tests and documentation.
  Owns:
  - `rstest` unit tests and `rstest-bdd` scenarios under `backend/tests/*`;
  - feature files under `backend/tests/features/*`;
  - architecture decision updates in
    `docs/wildside-backend-architecture.md`;
  - roadmap checkbox completion in `docs/backend-roadmap.md` at finish.

Coordination rules:

- Ownership is strict; agents do not edit files outside their area.
- Merge order was A -> B -> C -> D.
- Targeted tests were re-run after each merge point where feasible.
- Final quality gates were executed and captured in `/tmp` logs.

## Progress

- [x] (2026-02-24) Confirmed branch context and that `PLANS.md` is absent.
- [x] (2026-02-24) Loaded `execplans` and `hexagonal-architecture` guidance
      for this task.
- [x] (2026-02-24) Gathered roadmap scope and architecture constraints for
      3.4.1.
- [x] (2026-02-24) Used an explorer agent team to collect:
      roadmap details, architecture update points, testing constraints, and
      current code anchors/gaps.
- [x] (2026-02-24) Drafted this ExecPlan at
      `docs/execplans/backend-3-4-1-ingest-osm-command-line.md`.
- [x] (2026-02-24) Finalized the ingestion seam by introducing
      `OsmSourceRepository` and the outbound adapter
      `backend/src/outbound/osm_source.rs` backed by `wildside-data`.
- [x] (2026-02-24) Implemented domain-driven ingestion orchestration and
      deterministic rerun policy in `backend/src/domain/osm_ingestion.rs`.
- [x] (2026-02-24) Implemented provenance/rerun persistence via migration
      `backend/migrations/2026-02-24-000000_create_osm_ingestion_provenance`
      and Diesel adapter implementations.
- [x] (2026-02-24) Implemented `ingest-osm` CLI adapter and wiring in
      `backend/src/bin/ingest_osm.rs` and `backend/Cargo.toml`.
- [x] (2026-02-24) Added coverage artefacts:
      CLI helper unit tests in `backend/src/bin/ingest_osm.rs` and BDD
      scenarios in `backend/tests/osm_ingestion_bdd.rs` plus
      `backend/tests/features/osm_ingestion.feature`.
- [x] (2026-02-24) Recorded architecture design decisions for 3.4.1 in
      `docs/wildside-backend-architecture.md`.
- [x] (2026-02-24) Marked roadmap item 3.4.1 as done in
      `docs/backend-roadmap.md`.
- [x] (2026-02-24) Re-ran full quality gates on the final integrated branch:
      `make check-fmt`, `make lint`, and `make test` with retained logs.
- [x] (2026-02-25) Committed and pushed final integrated implementation after
      gate evidence was captured.

## Surprises & Discoveries

- Observation (2026-02-24): `wildside-engine` integration landed as a direct
  `wildside-data` dependency in `backend/Cargo.toml` rather than through shell
  invocation of `wildside-cli`.
  Impact: domain logic stays independent of process execution details, and
  parser integration is exercised through a typed outbound adapter.

- Observation (2026-02-24): behavioural coverage includes deterministic replay
  and missing-schema unhappy paths against temporary PostgreSQL instances.
  Impact: the main reliability risk shifted from behaviour modelling to final
  gate-evidence collection on the integrated branch.

- Observation (2026-02-24): `make test` initially failed on two BDD suites
  with transient embedded PostgreSQL startup failures under default nextest
  parallelism.
  Impact: rerunning with `NEXTEST_TEST_THREADS=1` stabilized the suite while
  preserving full test coverage.

## Decision Log

- Decision: keep this ExecPlan strictly scoped to roadmap item 3.4.1.
  Rationale: roadmap sequencing separates 3.4.1 foundational ingest delivery
  from 3.4.2/3.4.3 enrichment workers and reporting.
  Date/Author: 2026-02-24 / Codex.

- Decision: require explicit domain ports for provenance and deterministic rerun
  policy rather than embedding these rules in the CLI adapter.
  Rationale: preserves hexagonal boundaries and keeps business rules testable
  without CLI/persistence coupling.
  Date/Author: 2026-02-24 / Codex.

- Decision: use a four-agent ownership model (domain, persistence, CLI, tests)
  for implementation execution.
  Rationale: reduces cross-layer drift and keeps merge/testing sequence explicit.
  Date/Author: 2026-02-24 / Codex.

- Decision: treat `wildside-data` as the backend integration boundary for 3.4.1
  while keeping `wildside-cli ingest` as a reference capability in the shared
  upstream project.
  Rationale: crate-level integration keeps command orchestration testable and
  avoids subprocess coupling in domain flows.
  Date/Author: 2026-02-24 / Codex.

- Decision: keep roadmap item 3.4.1 marked complete and track remaining gate
  evidence and service-level unit coverage as explicit follow-up risk items in
  this plan.
  Rationale: delivered artefacts for CLI, ports, migration, and BDD are present
  in-repo, while final integrated verification can be completed by the main
  owner.
  Date/Author: 2026-02-24 / Codex.

## Context and orientation

Primary references:

- `docs/backend-roadmap.md` (section 3.4.1 scope and acceptance bullets).
- `docs/wildside-backend-architecture.md`:
  - domain/port boundaries;
  - driven-port decision history;
  - ingestion workflow and provenance expectations.
- `docs/rust-testing-with-rstest-fixtures.md`.
- `docs/rstest-bdd-users-guide.md`.
- `docs/pg-embed-setup-unpriv-users-guide.md`.
- `docs/rust-doctest-dry-guide.md`.
- `docs/complexity-antipatterns-and-refactoring-strategies.md`.

Current implementation anchors:

- CLI inbound adapter:
  - `backend/src/bin/ingest_osm.rs`;
  - `backend/Cargo.toml` (`[[bin]] ingest-osm` wiring and `wildside-data`
    dependency).
- Domain command and ports:
  - `backend/src/domain/osm_ingestion.rs`;
  - `backend/src/domain/ports/osm_ingestion_command.rs`;
  - `backend/src/domain/ports/osm_source_repository.rs`;
  - `backend/src/domain/ports/osm_poi_repository.rs`;
  - `backend/src/domain/ports/osm_ingestion_provenance_repository.rs`.
- Outbound adapters and schema:
  - `backend/src/outbound/osm_source.rs`;
  - `backend/src/outbound/persistence/diesel_osm_poi_repository.rs`;
  - `backend/src/outbound/persistence/diesel_osm_ingestion_provenance_repository.rs`;
  - `backend/src/outbound/persistence/schema.rs`;
  - `backend/migrations/2026-02-24-000000_create_osm_ingestion_provenance/`.
- Coverage artefacts:
  - `backend/tests/osm_ingestion_bdd.rs`;
  - `backend/tests/features/osm_ingestion.feature`;
  - CLI helper unit tests embedded in `backend/src/bin/ingest_osm.rs`.

## Milestones

### Milestone 1: Integration contract and domain boundary design

Define the backend-owned ingestion orchestration contract before code changes:

- decide how `wildside-engine` ingestion capabilities are invoked from this
  workspace;
- define typed domain inputs/outputs for geofence-filtered ingest runs;
- define provenance and deterministic rerun key semantics in domain language.

Deliverables:

- documented contract notes in this ExecPlan (`Decision Log` + `Progress`);
- domain port changes staged for review before adapter implementation.

Acceptance checks:

- all new contracts are domain-owned and free of persistence/CLI framework
  types;
- clear rerun key definition (`geofence_id + input_digest`) is present.

### Milestone 2: Persistence model and migration delivery

Implement provenance and deterministic rerun persistence through outbound
adapters:

- add migration(s) for provenance storage and rerun key constraints/indexes;
- update Diesel schema/model mappings;
- implement/extend outbound adapters behind domain ports.

Deliverables:

- migration SQL (`up.sql` and `down.sql`);
- adapter implementations and typed error mapping;
- unit tests for insert/replay/conflict/error paths.

Acceptance checks:

- duplicate ingest attempts with the same key are deterministic;
- provenance rows contain source URL, digest, timestamp, and bounding box;
- persistence behaviour is accessible only via domain ports.

### Milestone 3: CLI adapter implementation (`ingest-osm`)

Create the Rust CLI inbound adapter that orchestrates ingest runs:

- implement command parsing and required arguments (input source, geofence
  selector, provenance metadata source);
- execute geofence filtering before persistence orchestration;
- invoke domain services/ports and emit deterministic success/error output.

Deliverables:

- CLI entrypoint module and wiring;
- mapping layer between CLI arguments and domain command structures;
- user-facing command help and error semantics.

Acceptance checks:

- CLI can run against embedded PostgreSQL test setup;
- failure modes (invalid geofence, bad input, persistence errors) are typed and
  tested;
- no direct SQL or Diesel calls from CLI module.

### Milestone 4: Behavioural and unit test coverage

Add complete test coverage per roadmap requirements:

- `rstest` unit tests for domain services, adapter mapping, and edge handling;
- `rstest-bdd` scenarios for end-to-end ingest behaviour and reruns;
- unhappy-path scenarios (invalid payload, schema missing, deterministic rerun
  collisions, geofence mismatch).

Deliverables:

- new/updated test modules in `backend/tests/*`;
- `.feature` files under `backend/tests/features/*`;
- fixture reuse via embedded PostgreSQL support modules.

Acceptance checks:

- happy, unhappy, and edge paths are covered in both unit and behavioural
  suites;
- embedded PostgreSQL harness (`pg-embedded-setup-unpriv`) is used for local
  integration behaviour.

### Milestone 5: Documentation and roadmap completion

Update project documentation after implementation stability:

- append 3.4.1 design decision entries in
  `docs/wildside-backend-architecture.md` near driven-port/ingestion sections;
- update roadmap checkbox in `docs/backend-roadmap.md` from `[ ]` to `[x]`
  only after all gates pass.

Acceptance checks:

- architecture docs capture what changed and why;
- roadmap reflects completion only with evidence-backed gates.

### Milestone 6: Final validation, evidence, and commit

Run the full required gate stack and capture evidence logs:

```shell
project="$(get-project)"
branch="$(git branch --show)"
set -o pipefail
make check-fmt 2>&1 | tee "/tmp/check-fmt-${project}-${branch}.out"
make lint 2>&1 | tee "/tmp/lint-${project}-${branch}.out"
make test 2>&1 | tee "/tmp/test-${project}-${branch}.out"
```

If test runtime stability requires it:

```shell
set -o pipefail
NEXTEST_TEST_THREADS=1 make test 2>&1 | tee "/tmp/test-${project}-${branch}.out"
```

Acceptance checks:

- all three gates succeed;
- logs are retained under `/tmp/*-${project}-${branch}.out`;
- only then commit with a descriptive, imperative message.

## Outcomes & Retrospective

Delivered outcomes:

- Shipped `ingest-osm` as a backend-owned CLI command that parses input source,
  geofence identity, geofence bounds, and source provenance metadata, then
  delegates orchestration through the `OsmIngestionCommand` driving port.
- Implemented geofence filtering and deterministic rerun semantics in
  `OsmIngestionCommandService`, with replay detection keyed by
  `(geofence_id, input_digest)`.
- Added persistence support for rerun/provenance metadata via a dedicated table
  and Diesel adapter wiring.
- Added integration to `wildside-data` via `WildsideDataOsmSourceRepository`
  so upstream OSM parsing remains outside domain logic.
- Added behavioural scenarios that exercise executed ingests, deterministic
  replay, and missing-schema failures against temporary PostgreSQL databases.
- Updated architecture and roadmap docs to reflect 3.4.1 completion status.

Gate evidence:

- [x] `make check-fmt`:
      `/tmp/check-fmt-wildside-backend-3-4-1-ingest-osm-command-line.out`.
- [x] `make lint`:
      `/tmp/lint-wildside-backend-3-4-1-ingest-osm-command-line.out`.
- [x] `make test` initial run (captured transient BDD cluster startup failures):
      `/tmp/test-wildside-backend-3-4-1-ingest-osm-command-line.out`.
- [x] `NEXTEST_TEST_THREADS=1 make test` stabilization run (passed):
      `/tmp/test-threads1-wildside-backend-3-4-1-ingest-osm-command-line.out`.

Retrospective notes:

- The port split (`OsmSourceRepository`, `OsmPoiRepository`,
  `OsmIngestionProvenanceRepository`) kept backend ownership clear while
  constraining SQL and parser details to outbound adapters.
- Deterministic rerun behaviour is now explicitly enforceable through both
  schema-level and service-level contracts.
- The remaining risk is environmental: low free space on `/data` can destabilize
  large rebuilds. `cargo clean` before full-suite reruns remains advisable when
  free space drops below 5 GiB.
