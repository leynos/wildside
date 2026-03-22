# Implement the pagination crate foundation (roadmap 4.1.1)

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises &
Discoveries`, `Decision Log`, and `Outcomes & Retrospective` must be kept up
to date as work proceeds.

Status: DRAFT

This plan covers roadmap item 4.1.1 only:
`Implement backend/crates/pagination providing opaque cursor encoding,
PageParams, and Paginated<T> envelopes with navigation links, backed by unit
tests for cursor round-tripping.`

## Purpose / big picture

Wildside needs one reusable pagination foundation before any endpoint can move
from ad hoc page tokens to a shared cursor contract. After this change, the
workspace will contain a dedicated `pagination` crate that exposes:

- opaque cursor encoding and decoding for stable ordered queries;
- `PageParams` parsing and normalization with default and maximum limits;
- a generic `Paginated<T>` response envelope with `self`, `next`, and `prev`
  links.

Observable success for roadmap 4.1.1 is narrow and concrete:

- `backend/crates/pagination` exists as a workspace member and builds cleanly.
- The crate exposes documented public types for cursor handling,
  `PageParams`, and paginated envelopes.
- `rstest` unit tests prove cursor round-tripping and unhappy-path decode
  failures.
- `rstest-bdd` behavioural tests prove public crate behaviour for happy,
  unhappy, and edge cases without coupling the foundation crate to backend
  persistence.
- `docs/wildside-backend-architecture.md` records the packaging and boundary
  decisions for the shared pagination crate.
- `docs/backend-roadmap.md` is marked done for 4.1.1 only after the
  implementation is complete and all required gates pass.
- `make check-fmt`, `make lint`, and `make test` succeed, with output
  captured through `tee`.

## Constraints

- Scope is roadmap item 4.1.1 only. Do not implement endpoint adoption from
  4.2.x, OpenAPI rollout from 4.3.x, or the broader direction/property-test
  expansion from 4.1.2 unless a minimal seam is strictly required for 4.1.1.
- Preserve hexagonal boundaries:
  - the new crate must stay transport- and persistence-agnostic;
  - it must not depend on Actix, Diesel, or backend-specific repository code;
  - endpoint-specific HTTP mapping remains in the backend crate.
- Follow the design intent in `docs/keyset-pagination-design.md`, but keep the
  implementation limited to the foundation types named in roadmap 4.1.1.
- The crate path must remain `backend/crates/pagination` as named by the
  roadmap and design document.
- Because the root workspace currently auto-discovers `crates/*` rather than
  `backend/crates/*`, add the new crate as an explicit workspace member in the
  root [Cargo.toml](/home/user/project/Cargo.toml).
- Keep the public API small and human-readable. Split code into small modules
  so no file exceeds the 400-line project limit.
- Add module-level and public-item Rustdoc comments. Use examples where they
  materially clarify usage, but do not try to complete roadmap 4.1.3 in this
  change.
- Use `rstest` for unit tests and `rstest-bdd` for behavioural tests.
- Use `pg-embed-setup-unpriv` in the local verification flow through
  `make prepare-pg-worker` and `make test`, but do not invent database
  dependencies inside the pagination crate itself merely to satisfy a tooling
  requirement.
- Record new design decisions in
  [docs/wildside-backend-architecture.md](/home/user/project/docs/wildside-backend-architecture.md).
- Update documentation in en-GB-oxendict style.

## Tolerances

- Packaging tolerance: if `backend/crates/pagination` cannot be included
  cleanly as an explicit workspace member, stop and resolve the workspace
  structure before writing feature code.
- Scope tolerance: if 4.1.1 requires edits in more than 12 files or more than
  roughly 900 net lines, stop and split work so 4.1.2 or 4.2.x does not leak
  in.
- API tolerance: if the crate needs endpoint-specific concepts such as Actix
  extractors, HTTP status codes, Diesel traits, or admin provenance-specific
  cursor formats, stop and escalate because the abstraction boundary has
  failed.
- Behaviour tolerance: if navigation links cannot be expressed without fully
  implementing 4.1.2 traversal helpers, land the minimal direction seam needed
  for link generation and defer the rest explicitly.
- Test tolerance: if behavioural coverage cannot be expressed at crate level
  without real persistence, stop and revisit the acceptance test shape rather
  than forcing database coupling into the crate.
- Gate tolerance: if `make check-fmt`, `make lint`, or `make test` fails after
  three repair loops, stop and capture the failure logs.
- Environment tolerance: if embedded PostgreSQL setup blocks `make test`,
  verify `/dev/null`, `PG_EMBEDDED_WORKER`, and `make prepare-pg-worker`
  first; if the environment still fails, record evidence instead of masking
  the issue.

## Risks

- Risk: the roadmap says `backend/crates/pagination`, but the current workspace
  layout only auto-discovers `crates/*`.
  Severity: high.
  Likelihood: high.
  Mitigation: treat explicit workspace wiring as the first implementation step
  and document the decision in the architecture record.

- Risk: 4.1.1 and 4.1.2 overlap conceptually because navigation links imply
  directional cursors.
  Severity: medium.
  Likelihood: high.
  Mitigation: limit 4.1.1 to the smallest public seam needed for opaque cursor
  round-tripping and link generation, and defer stronger invariants and
  property tests to 4.1.2.

- Risk: endpoint-specific defaults could leak into the shared crate.
  Severity: medium.
  Likelihood: medium.
  Mitigation: keep shared defaults at 20 and 100 in the crate, but document
  that consuming endpoints may layer narrower bounds on top, as already noted
  in the backend architecture document for admin provenance reporting.

- Risk: requiring behavioural tests plus `pg-embed-setup-unpriv` could tempt
  implementers to add pointless database coverage to a pure utility crate.
  Severity: medium.
  Likelihood: medium.
  Mitigation: keep behavioural tests focused on the crate's observable API and
  rely on the repository-wide `make test` gate to exercise embedded PostgreSQL
  setup in the wider workspace.

- Risk: malformed cursor handling could be under-specified, causing later
  endpoint-specific error mapping churn.
  Severity: medium.
  Likelihood: medium.
  Mitigation: define a narrow crate-local error enum now and document that HTTP
  `400` translation remains backend work for roadmap 4.2.2.

## Progress

- [x] (2026-03-22 00:00Z) Reviewed roadmap item 4.1.1, the keyset pagination
  design, backend architecture guidance, testing guidance, and existing
  ExecPlan conventions.
- [x] (2026-03-22 00:00Z) Confirmed that `backend/crates/` does not yet exist
  and that the root workspace must be updated explicitly for this crate path.
- [x] (2026-03-22 00:00Z) Drafted this ExecPlan at
  [docs/execplans/backend-4-1-1-pagination-crate.md](/home/user/project/docs/execplans/backend-4-1-1-pagination-crate.md).
- [ ] Approval gate: wait for explicit approval before implementation begins.
- [ ] Create and wire the new workspace crate.
- [ ] Implement cursor, params, envelope, and error modules with Rustdoc.
- [ ] Add `rstest` unit coverage and `rstest-bdd` behavioural coverage.
- [ ] Update backend architecture documentation with the shared-crate design
  decisions.
- [ ] Run formatting, lint, Markdown, and test gates with retained logs.
- [ ] Mark roadmap item 4.1.1 done only after the implementation and gates are
  complete.

## Surprises & discoveries

- Observation: there is currently no `backend/crates/` directory.
  Evidence: `find backend/crates -maxdepth 3 -type f` returned
  `No such file or directory`.
  Impact: the implementation must create the directory tree and cannot assume
  an existing crate template.

- Observation: the root workspace currently lists members manually and
  auto-discovery only targets `crates/*`, not `backend/crates/*`.
  Evidence: [Cargo.toml](/home/user/project/Cargo.toml) contains
  `members = ["backend", "crates/example-data", "tools/architecture-lint"]`
  and `workspace.metadata.autodiscover.globs = ["apps/*", "crates/*", ...]`.
  Impact: the new pagination crate must be added explicitly or it will be
  invisible to workspace commands and quality gates.

- Observation: the backend architecture document already reserves special
  pagination compatibility rules for admin provenance reporting.
  Evidence:
  [docs/wildside-backend-architecture.md](/home/user/project/docs/wildside-backend-architecture.md)
  around the "3.4.3 Pagination Compatibility Requirements" section.
  Impact: the foundation crate must stay generic and must not encode admin
  provenance-specific `before` semantics.

- Observation: repository-wide test execution depends on
  `pg-embed-setup-unpriv` via `make prepare-pg-worker` and
  `make test`, not on per-crate direct cluster setup.
  Evidence: [Makefile](/home/user/project/Makefile) targets
  `prepare-pg-worker` and `test-rust`, plus the
  `docs/pg-embed-setup-unpriv-users-guide.md` guidance.
  Impact: local verification for 4.1.1 must include the standard gate flow,
  but the new crate should remain free of direct database setup code.

## Decision Log

- Decision: implement the crate at `backend/crates/pagination` exactly as the
  roadmap and design document state, and wire it into the workspace explicitly
  from the root manifest.
  Rationale: changing the path at plan time would silently rewrite approved
  product scope; the cleaner fix is explicit workspace membership.
  Date/Author: 2026-03-22 / planning team.

- Decision: keep the pagination crate generic and infrastructure-neutral.
  Rationale: under the project's hexagonal rules, reusable pagination
  primitives belong outside endpoint adapters and must not depend on Actix,
  Diesel, or backend-specific repository types.
  Date/Author: 2026-03-22 / planning team.

- Decision: treat directional cursor behaviour as a minimal seam in 4.1.1 only
  if it is strictly required to generate `next` and `prev` links, while
  deferring richer helpers and property tests to roadmap 4.1.2.
  Rationale: this matches the roadmap split while avoiding an artificial
  foundation that would need immediate rework.
  Date/Author: 2026-03-22 / planning team.

- Decision: satisfy the behavioural-test requirement at the crate boundary,
  not by adding fake persistence to the pagination crate.
  Rationale: the feature under construction is a pure utility crate; database
  coverage belongs to endpoint adoption work, while embedded PostgreSQL still
  remains part of the repo-wide gate flow.
  Date/Author: 2026-03-22 / planning team.

- Decision: do not mark roadmap 4.1.1 done during planning.
  Rationale: the roadmap may only change once implementation is complete and
  the full gate suite is green.
  Date/Author: 2026-03-22 / planning team.

## Outcomes & retrospective

Planning complete. Implementation has not started yet.

The expected completed state is:

- a reusable workspace crate at `backend/crates/pagination`;
- documented public API for opaque cursors, page parameters, and paginated
  envelopes;
- `rstest` unit tests and `rstest-bdd` behavioural tests covering happy,
  unhappy, and edge behaviour;
- an updated backend architecture decision record;
- roadmap item 4.1.1 marked done only after green gates.

## Agent team and ownership

This implementation should be executed by the following agent team. One person
may cover multiple roles, but the ownership boundaries should remain visible.

- Coordinator agent:
  owns sequencing, keeps this ExecPlan current, enforces tolerances, and
  decides when the work is ready for roadmap closure.
- Crate design agent:
  creates the new crate layout, selects module boundaries, and keeps the
  public API small, documented, and transport-neutral.
- Test agent:
  owns `rstest` unit coverage, `rstest-bdd` behavioural coverage, and the
  verification transcript for happy, unhappy, and edge cases.
- Documentation agent:
  updates
  [docs/wildside-backend-architecture.md](/home/user/project/docs/wildside-backend-architecture.md)
  and later
  [docs/backend-roadmap.md](/home/user/project/docs/backend-roadmap.md) after
  implementation is complete.
- Gate agent:
  runs `make prepare-pg-worker`, `make fmt`, `make markdownlint`,
  `make nixie`, `make check-fmt`, `make lint`, and `make test` with
  `set -o pipefail` and `tee`, then captures the log paths.

Coordination sequence:

1. Coordinator agent confirms approval and keeps the plan current.
2. Crate design agent lands the workspace/member wiring and failing unit
   seams.
3. Test agent locks the public behaviour in unit and BDD tests.
4. Crate design agent completes the implementation until tests pass.
5. Documentation agent records design decisions and closes roadmap 4.1.1 only
   after the gates are green.
6. Gate agent performs the final verification run and records evidence.

## Context and orientation

Primary references to load before making edits:

- [docs/backend-roadmap.md](/home/user/project/docs/backend-roadmap.md)
- [docs/keyset-pagination-design.md](/home/user/project/docs/keyset-pagination-design.md)
- [docs/wildside-backend-architecture.md](/home/user/project/docs/wildside-backend-architecture.md)
- [docs/rust-testing-with-rstest-fixtures.md](/home/user/project/docs/rust-testing-with-rstest-fixtures.md)
- [docs/rstest-bdd-users-guide.md](/home/user/project/docs/rstest-bdd-users-guide.md)
- [docs/rust-doctest-dry-guide.md](/home/user/project/docs/rust-doctest-dry-guide.md)
- [docs/complexity-antipatterns-and-refactoring-strategies.md](/home/user/project/docs/complexity-antipatterns-and-refactoring-strategies.md)
- [docs/pg-embed-setup-unpriv-users-guide.md](/home/user/project/docs/pg-embed-setup-unpriv-users-guide.md)

Current code anchors and likely edit targets:

- [Cargo.toml](/home/user/project/Cargo.toml) for workspace membership.
- [backend/Cargo.toml](/home/user/project/backend/Cargo.toml) only if a local
  path dependency becomes necessary during implementation; avoid touching it
  for 4.1.1 unless required.
- `backend/crates/pagination/Cargo.toml` as the new crate manifest.
- `backend/crates/pagination/src/lib.rs` plus small supporting modules such as
  `cursor.rs`, `params.rs`, `envelope.rs`, and `error.rs`.
- `backend/crates/pagination/tests/` for `rstest` and `rstest-bdd` coverage.
- [docs/wildside-backend-architecture.md](/home/user/project/docs/wildside-backend-architecture.md)
  for the shared-crate decision record.
- [docs/backend-roadmap.md](/home/user/project/docs/backend-roadmap.md) for
  final checkbox closure only.

Deliberate non-goals for this item:

- no endpoint wiring in
  [backend/src/inbound/http/admin_enrichment.rs](/home/user/project/backend/src/inbound/http/admin_enrichment.rs)
  or `/api/users` yet;
- no repository filter logic;
- no OpenAPI adoption;
- no telemetry fields;
- no legacy `before` compatibility migration work.

## Milestones

### Milestone 0 - Create the crate seam and red-state the API

Create the new directory tree and manifest at `backend/crates/pagination`,
update the root workspace membership, and add a small crate-level module
layout. Start with failing tests that define the public contract:

- cursor encode/decode round-trip for a representative key struct;
- malformed base64 and malformed JSON decode failures;
- `PageParams` defaulting and maximum-limit normalization;
- envelope link generation that preserves `self` and omits unavailable
  navigation links cleanly.

Acceptance for this milestone is a compiling crate skeleton plus failing tests
that describe the missing behaviour precisely.

### Milestone 1 - Implement cursor and error primitives

Implement the cursor module first. The recommended public shape is:

- `Direction` only if required for 4.1.1 link generation;
- `Cursor<K>` with opaque base64url JSON encode/decode helpers;
- a narrow `PaginationError` enum for invalid cursor payloads and related
  normalization failures.

Keep the implementation free of panicking encode/decode paths. Use
`serde`, `serde_json`, and base64url-safe encoding. If URL construction needs
query-safe handling, use the existing `url` crate rather than hand-rolled
string concatenation logic.

Acceptance for this milestone is green unit coverage for round-tripping and
decode errors.

### Milestone 2 - Implement page params and envelope behaviour

Add `PageParams` and `Paginated<T>` with the smallest useful supporting types.
The recommended shape is:

- `PageParams { cursor: Option<String>, limit: Option<u32> }`;
- limit normalization helpers that default to 20 and cap at 100;
- `PaginationLinks { self, next, prev }`;
- `Paginated<T> { data, limit, links }`.

The crate may expose helper constructors so callers can assemble envelopes
without repeating link and limit logic. Keep these helpers generic; they should
not know anything about specific endpoints, repositories, or HTTP frameworks.

Acceptance for this milestone is green unit coverage for normalization and
link-envelope assembly.

### Milestone 3 - Add behavioural coverage and documentation

Add `rstest-bdd` scenarios around the crate's public behaviour. These should
exercise observable examples such as:

- a valid cursor token round-trips through encode and decode;
- an invalid token is rejected with a typed error;
- a page request without a limit uses the default;
- an oversized limit is bounded;
- a paginated response with only one available direction emits only the
  expected links.

Add Rustdoc comments for all public items and a concise crate-level `//!`
overview that states the stable-ordering requirement and the shared default and
maximum limits. Keep this concise so roadmap 4.1.3 still has clear remaining
scope.

Acceptance for this milestone is green unit and BDD coverage plus passing
Rustdoc/lint checks for the new public API.

### Milestone 4 - Record decisions and run full gates

Update the backend architecture document with the pagination crate decisions:

- why the shared crate lives at `backend/crates/pagination`;
- why it stays framework-neutral;
- how shared defaults interact with narrower endpoint-specific limits later.

Then run the full verification flow with retained logs. Use commands in this
shape so failures are not hidden by pipes:

```bash
mkdir -p /tmp/backend-4-1-1
set -o pipefail && make prepare-pg-worker 2>&1 | tee /tmp/backend-4-1-1/prepare-pg-worker.log
set -o pipefail && make fmt 2>&1 | tee /tmp/backend-4-1-1/fmt.log
set -o pipefail && make markdownlint 2>&1 | tee /tmp/backend-4-1-1/markdownlint.log
set -o pipefail && make nixie 2>&1 | tee /tmp/backend-4-1-1/nixie.log
set -o pipefail && make check-fmt 2>&1 | tee /tmp/backend-4-1-1/check-fmt.log
set -o pipefail && make lint 2>&1 | tee /tmp/backend-4-1-1/lint.log
set -o pipefail && make test 2>&1 | tee /tmp/backend-4-1-1/test.log
```

If every required gate passes, mark 4.1.1 done in
[docs/backend-roadmap.md](/home/user/project/docs/backend-roadmap.md) and
update this ExecPlan's status and progress sections.

## Acceptance checks

Use these observable checks during implementation:

1. `cargo test -p pagination` passes and includes the new `rstest` unit tests.
2. `cargo test -p pagination --test pagination_foundation_bdd` passes and
   exercises the new Gherkin scenarios.
3. `cargo doc --workspace --no-deps` passes with the new crate present.
4. `make check-fmt`, `make lint`, and `make test` pass with logs captured.
5. The roadmap checkbox for 4.1.1 changes from unchecked to checked only after
   the gate logs are green.
