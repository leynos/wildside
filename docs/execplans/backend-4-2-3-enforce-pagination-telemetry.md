# Enforce pagination telemetry for page size, direction, and traversal

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, `Outcomes & Retrospective`, `Conformance Basis`, and
`Verification Plan` must be kept up to date as work proceeds.

Status: DRAFT

## Purpose / big picture

Roadmap task 4.2.3 requires pagination telemetry that records page size,
cursor direction, and page traversal counts for analytics. Today the backend
records pagination *failures* only: `wildside_pagination_errors_total`
counts rejected cursors. Nothing records a *successful* page delivery, so no
one can answer how large the pages clients ask for are, whether clients
navigate forwards or backwards, or how many pages a traversal covers before
it stops.

After this change an operator can answer all three questions from the
Prometheus exposition, and an engineer can answer them from structured logs
even in a build without the `metrics` cargo feature.

The completed work is observable through the following.

1. Requesting `GET /api/v1/users` and then scraping `/metrics` shows
   `wildside_pagination_pages_total` incremented with
   `traversal="first"`, and `wildside_pagination_rows_returned_total`
   incremented by the number of rows the response carried.
2. Following the `links.next` URL increments the same counter with
   `traversal="next"`; following `links.prev` increments it with
   `traversal="prev"`.
3. Reaching the final page increments the counter with `outcome="terminal"`;
   every earlier page carries `outcome="continuing"`.
4. Omitting `limit` records `limit_source="default"`; supplying one records
   `limit_source="explicit"`.
5. A build without the `metrics` feature still emits one structured
   `tracing` event per page delivery carrying the same fields.
6. `make check-fmt`, `make lint`, and `make test` all pass from the
   repository root, with output captured under `/tmp`.

The key insight, established by the design review recorded in
`Decision Log`, is that cursor pagination has no page number, and the
temptation is therefore to carry a traversal-depth counter inside the opaque
cursor. That approach was examined in detail and rejected on six independent
grounds. This plan instead records counters from which mean traversal depth
is derived in the query layer, which satisfies the roadmap wording without
touching a shared wire format.

## Constraints

- This plan must remain a draft until explicitly approved. Opening the plan
  pull request authorizes no implementation.
- **Sequencing.** This work must land *after*
  `docs/execplans/competing-metrics-patterns.md` (branch
  `feat/competing-metrics-patterns`), which establishes the single
  sanctioned metrics pattern and **deletes**
  `backend/src/observability/pagination_errors.rs`. This plan builds on that
  pattern and must not reintroduce the module-level global it removes. If
  that plan has not landed when implementation begins, stop and escalate.
- The metrics pattern is fixed by that approved plan and must be followed
  exactly: a port trait in `backend/src/domain/ports/`, a `NoOp*`
  implementation, a `Prometheus*` adapter in `backend/src/outbound/metrics/`
  taking `&prometheus::Registry` at construction, and selection at the
  composition root in `backend/src/server/mod.rs`. No process-global metric
  state, and no `OnceLock`.
- Preserve the hexagonal dependency rule from
  `docs/wildside-backend-architecture.md` and the `hexagonal-architecture`
  skill. Domain code must not import `prometheus` or `actix-web-prom`.
- `backend/src/inbound/http/users.rs::list_users` must remain a thin
  coordinator. It may parse query parameters, call ports, and build the
  response envelope; it must not import Diesel, bb8, or outbound modules.
- The `GET /api/v1/users` response envelope from roadmap 4.2.1 must not
  change: `{ "data": […], "limit": N, "links": { "self": …, "next": …,
  "prev": … } }`.
- The opaque cursor wire format must not change. No new fields, no new
  constructors on `pagination::Cursor`, and no regeneration of the snapshot
  fixtures under `backend/crates/pagination/src/cursor/snapshots/`.
- Existing error semantics must not change: unauthenticated requests return
  `401`, invalid cursors `400`, repository connection failures `503`, and
  unexpected query failures a redacted `500`.
- Telemetry must never alter a user-visible response. The recording method
  returns `()`; a metrics failure cannot produce a non-2xx status.
- Metric label sets must be closed at compile time. Every label value comes
  from a fieldless enum with a `const fn as_label`.
- Label *order* is part of the metric contract. `with_label_values` matches
  values positionally against the declared names, so a transposition
  silently mislabels every series while leaving the series count correct.
  The adapter must declare and populate labels in one place, and
  V-LABEL-CORRECT asserts the pairing.
- Call sites invoke `record`, never `increment`. `record` is the provided
  method that emits the structured log before delegating; calling
  `increment` directly would silently drop the log in every build. This is
  the whole point of the approved provided-method idiom.
- New or updated documentation must use en-GB Oxford spelling, wrap prose at
  80 columns and code at 120, and follow
  `docs/documentation-style-guide.md`.
- No source file may exceed 400 lines (`AGENTS.md`, enforced by Whitaker
  `module_max_lines`).
- Each milestone is committed separately after its gates pass. Do not commit
  a failing gate. Use `tee` for all gate commands.

## Tolerances (exception triggers)

- Scope: stop and escalate if implementation touches more than 24 files or
  exceeds roughly 900 net source lines, excluding Gherkin feature text,
  generated OpenAPI output, and this ExecPlan.
- Sequencing: stop and escalate if `competing-metrics-patterns` has not
  landed, or if it landed with a materially different port shape from the
  one recorded in `Interfaces and dependencies` below.
- Wire format: stop and escalate if any change appears to require editing
  `backend/crates/pagination/src/cursor/`, its snapshots, or its trybuild
  golden files. That is the signal that depth-in-cursor has crept back in.
- Public interface: stop and escalate before changing the successful
  `GET /api/v1/users` JSON shape or the top-level error envelope.
- Dependencies: `googletest` is the one new dev-dependency this plan adds
  (see `Decision Log`). Stop and escalate before adding any other crate,
  build tool, or service.
- Deployment: stop and escalate if closing the public `/metrics` path
  requires changes beyond `deploy/docker/backend.Dockerfile`,
  `deploy/charts/wildside/templates/ingress.yaml`, and one new
  ServiceMonitor template.
- Cardinality: stop and escalate if any proposed label value is not drawn
  from a fieldless enum, or if worst-case series per endpoint exceeds 32.
- Iterations: if a gate still fails after three focused fix attempts on the
  same failure, record it in `Surprises & Discoveries` and escalate with the
  log path.
- Ambiguity: stop and present options if "page traversal counts" is read by
  a reviewer as requiring a per-request depth distribution rather than
  counters. That reading reopens the rejected cursor change and is a
  scope decision, not an implementation detail.

## Risks

- Risk: `competing-metrics-patterns` and this plan both modify
  `backend/src/inbound/http/users_pagination.rs` and the composition block
  in `backend/src/server/mod.rs`, so a merge conflict is near-certain if
  they proceed in parallel. Severity: medium. Likelihood: high.
  Mitigation: the sequencing constraint above; rebase on that branch before
  starting, and treat its port shape as authoritative.

- Risk: enabling `--features metrics` in the production image exposes
  `/metrics` publicly. `deploy/charts/wildside/templates/ingress.yaml`
  routes the `/` prefix to container port 8080, and `actix-web-prom` mounts
  `/metrics` on the same Actix application. Unauthenticated scrape output
  would leak request patterns and internal counters. Severity: high.
  Likelihood: certain if unmitigated.
  Mitigation: EP-M6 adds an ingress rule that does not route `/metrics`,
  plus a ServiceMonitor scraping the ClusterIP service in-cluster. **This
  mitigation was selected without an explicit answer from the requester and
  must be confirmed at plan approval.** If a separate metrics listener on
  its own port is preferred instead, EP-M6 changes shape and grows.

- Risk: the adapter is written, tested, and never constructed in production
  wiring. This has already happened twice: `PrometheusRouteQueueMetrics` and
  `PrometheusEnrichmentJobMetrics` are constructed only in their own tests.
  Severity: high. Likelihood: medium.
  Mitigation: V-WIRED makes a real `/metrics` scrape from a constructed
  server a merge-blocking obligation, and EP-M5 extends the existing
  composition suites. No milestone is complete on unit tests alone.

- Risk: metric registration failure is handled inconsistently.
  `backend/src/server/mod.rs:106-110` treats it as fatal, while
  `backend/src/main.rs:41-50` warns and returns `None`, which makes
  `MetricsLayer::Disabled` drop the `/metrics` endpoint entirely while
  readiness still reports healthy. New telemetry would inherit whichever
  branch it happens to land in. Severity: medium. Likelihood: medium.
  Mitigation: EP-M5 states the policy explicitly and follows the fatal
  branch, on the grounds that registration failures are deterministic
  configuration bugs that should fail fast in staging.

- Risk: `backend/src/server/state_builders.rs` is 383 lines against the
  400-line cap, and is exactly where composition wiring lands. Severity:
  medium. Likelihood: high.
  Mitigation: EP-M0 splits it as a separate atomic refactor before any
  wiring is added, per the `AGENTS.md` separate-refactor rule.

- Risk: `build_users_page_response` already takes exactly four arguments and
  `clippy.toml` sets `too-many-arguments-threshold = 4`. Passing a telemetry
  handle as a fifth positional parameter fails lint immediately. Severity:
  low. Likelihood: certain if attempted.
  Mitigation: EP-M4 introduces a parameter object rather than a fifth
  argument.

- Risk: reintroducing `googletest` reverses a decision recorded days earlier
  in `docs/execplans/backend-5-2-2-job-structs-for-generate-route-and-
  enrichment.md`, which added the crate and then deliberately removed it.
  Severity: low. Likelihood: certain.
  Mitigation: the requester reaffirmed the choice after being shown that
  evidence. EP-M6 records it as the standing convention in
  `docs/developers-guide.md` so the question stops oscillating.

- Risk: embedded PostgreSQL behavioural tests are slow and occasionally
  flaky in shared environments. Severity: low. Likelihood: medium.
  Mitigation: reuse `backend/tests/support/embedded_postgres.rs`.
  `users_list_pagination_bdd` is already in the `pg-embed` group in
  `.config/nextest.toml` with a 300-second timeout, so no runner
  configuration change is needed.

## Progress

- [x] (2026-08-16) Renamed the local branch to
  `backend-4-2-3-enforce-pagination-telemetry`.
- [x] (2026-08-16) Reconnaissance complete across the pagination crate, the
  users endpoint, the telemetry stack, and the repository gates.
- [x] (2026-08-16) Six-lens design review completed; depth-in-cursor
  rejected unanimously; findings recorded in `Decision Log` and
  `Surprises & Discoveries`.
- [x] (2026-08-16) Aligned to the approved metrics pattern in
  `docs/execplans/competing-metrics-patterns.md`.
- [ ] EP-M0: split `backend/src/server/state_builders.rs` below the
  400-line cap (separate atomic refactor, no behaviour change).
- [ ] EP-M1: `PaginationPageMetrics` port, label enums, observation type,
  and `NoOp` implementation (red tests first).
- [ ] EP-M2: `PrometheusPaginationPageMetrics` adapter.
- [ ] EP-M3: move `PageTraversal` into the domain and delete
  `UsersPageDirection`.
- [ ] EP-M4: inbound emission from the users endpoint via a parameter
  object.
- [ ] EP-M5: composition-root wiring, registration-failure policy, and the
  wiring-assertion tests.
- [ ] EP-M6: deployment change, documentation, and roadmap tick.

## Surprises & discoveries

- Observation: the production container never enabled the `metrics` feature.
  Evidence: `deploy/docker/backend.Dockerfile:18` runs
  `cargo build --locked --release --bin backend` with no `--features`, while
  `backend/Cargo.toml` sets `default = []`.
  Impact: every Prometheus counter in the repository, including
  `wildside_pagination_errors_total` from roadmap 4.2.2, is dead code in the
  shipped image. Meanwhile all gates build with `--all-features`, so CI is
  green on a configuration that never ships. EP-M6 closes this.

- Observation: `docs/repository-structure.md:608` states that `/metrics` is
  exposed and scraped via a ServiceMonitor.
  Evidence: no ServiceMonitor template exists in
  `deploy/charts/wildside/templates/`, and `deployment.yaml:71` exposes only
  `containerPort: 8080`.
  Impact: the documentation asserts a capability that does not exist, which
  is how the previous point survived unnoticed. EP-M6 corrects it and makes
  it true.

- Observation: `Cursor::decode` does not use the derived `Deserialize`
  implementation. It deserializes into a separate `CursorWire` struct and
  reassembles the `Cursor` by hand.
  Evidence: `backend/crates/pagination/src/cursor/mod.rs:86-91` and
  `:335-352`.
  Impact: a new field added to `Cursor` would be silently dropped on decode,
  and the round-trip property tests would pass vacuously because their
  strategies generate only `created_at`, `id`, and `direction`. This is one
  of the reasons depth-in-cursor was rejected.

- Observation: the trybuild golden file embeds the list of `Cursor`
  constructors.
  Evidence:
  `backend/crates/pagination/tests/ui/cursor_decode_no_deserialise_owned.stderr`
  contains `pagination::Cursor::<Key>::new` and
  `pagination::Cursor::<Key>::with_direction` inside a rustc note.
  Impact: any new constructor breaks a CI gate. Reinforces the constraint
  against touching the cursor crate.

- Observation: `[profile.release]` in the root `Cargo.toml` sets
  `codegen-units`, `lto`, and `opt-level` but not `overflow-checks`.
  Evidence: the profile block has three keys.
  Impact: integer arithmetic on any traversal counter would panic in debug
  and test builds and wrap silently in the shipped container, so tests could
  not observe production behaviour. A further reason depth was rejected.

- Observation: terminality is direction-specific and is not simply
  `!has_more`.
  Evidence: the dispatch table in
  `backend/src/inbound/http/users_pagination.rs:239-246` emits a previous
  cursor unconditionally on a `Next` traversal, and never on a `First`
  traversal regardless of `has_more`.
  Impact: `outcome` must be derived from the emitted boundary link for the
  traversal direction, not from `has_more`. This is V-OUTCOME-DERIVED.

## Decision log

- Decision: do not carry a traversal-depth counter inside the opaque cursor.
  Rationale: six independent objections, each sufficient alone. The cursor
  is explicitly unsigned and client-editable
  (`backend/crates/pagination/src/cursor/mod.rs:13-16`), so a forged
  `u32::MAX` permanently poisons a monotonic histogram `_sum` with one
  request. Depth has no defined meaning under `Prev`, because
  `boundary_cursor` mints both a next and a previous cursor from the same
  page. It breaks `Cursor`'s derived `PartialEq`, under which two cursors at
  the same sort position must compare equal. It is non-monotonic across a
  rolling deploy, biased low precisely when a dashboard is being watched.
  It would silently drop on decode given the `CursorWire` asymmetry, with
  vacuous round-trip tests. And it inverts the dependency direction of a
  leaf crate that roadmap 4.2.4 is about to give a second consumer. The
  roadmap asks for "page traversal counts", which counters satisfy directly.
  Date/Author: 2026-08-16, drafting agent, on unanimous design-review
  finding.

- Decision: derive mean traversal depth and mean page size in the query
  layer from two counters rather than recording histograms.
  Rationale: page size is a bounded small integer in `1..=100`, so
  `rate(rows_returned_total) / rate(pages_total)` is the standard idiom and
  costs 3 series instead of 18. Dropping both histograms takes worst-case
  cardinality from 57 to 21 series per endpoint, and removes two label
  slices that were provably constant: `page_size{limit_source="default"}` is
  always 20, and depth on a first page is always 1.
  Date/Author: 2026-08-16, drafting agent.

- Decision: follow the approved pattern from
  `docs/execplans/competing-metrics-patterns.md` exactly — a port in
  `domain::ports`, a `NoOp`, a Prometheus adapter in `outbound::metrics`
  taking `&Registry`, injected at the composition root.
  Rationale: that plan is the accepted resolution of the repository's two
  competing metrics patterns and explicitly deletes the module-level global
  this work would otherwise have reused. Building on the deleted pattern
  would be immediately obsolete. The design review reached the same
  conclusion independently, on the grounds that roadmap 4.2.4 to 4.2.8 give
  this port a second consumer and that per-registry construction removes the
  `#[serial]` tax the global imposes on tests.
  Date/Author: 2026-08-16, drafting agent.

- Decision: use the provided-method idiom from the approved plan — a
  required `increment` hook and a provided `record` that emits the
  structured log and then calls `increment` — but emit at `debug!` rather
  than `info!`.
  Rationale: the idiom keeps log-always, count-when-enabled semantics in one
  place and gives `NoOp` logging for free. The level differs because the
  approved plan's event fires on *errors*, which are rare, whereas this one
  fires on every successful page. At 500 requests per second across twenty
  pods a ~300-byte JSON line per request is roughly 259 GB per day; and
  because `tracing_subscriber::fmt` holds a global stdout mutex across
  serialization and the write syscall, it costs an estimated 25 to 50 times
  more CPU than the counter increment it accompanies. `debug!` keeps the
  fallback available without making it the default cost.
  Date/Author: 2026-08-16, drafting agent.

- Decision: the port is synchronous, infallible, and takes its observation
  by value.
  Rationale: matches `RouteQueueMetrics` and the approved plan. Counter
  increments cannot meaningfully fail, and a `Result` here would either be
  discarded at every call site — as every `IdempotencyMetrics` caller does
  with `let _ =` — or invite a `?` that turns a dropped metric into a failed
  read request. With `endpoint` as an enum every field is `Copy`, so the
  observation is around 32 bytes and passing by value avoids the pointer
  indirection that `IdempotencyMetricLabels` needs only because it holds
  `String`.
  Date/Author: 2026-08-16, drafting agent.

- Decision: move `PageTraversal` into the domain and delete
  `UsersPageDirection` rather than defining a second enum.
  Rationale: `backend/src/inbound/http/users_pagination.rs:89-97` already
  defines exactly `First | Next | Prev` with the same intended label
  strings, and it has no users outside that module. `AGENTS.md` requires
  sweeping for an existing equivalent before adding an abstraction. Two
  copies would drift, and roadmap 4.2.4 would need a third or a conversion.
  Date/Author: 2026-08-16, drafting agent.

- Decision: `LimitSource` has three variants, `Default`, `Explicit`, and
  `Clamped`.
  Rationale: `normalize_limit` in
  `backend/crates/pagination/src/params.rs:98-104` clamps with
  `value.min(MAX_LIMIT)`. The users adapter pre-rejects oversized limits
  with a 400, so the clamp is unreachable there today, but the crate
  contract promises clamping and roadmap 4.2.4 to 4.2.6 consume `PageParams`
  directly. A two-variant enum would permanently discard "how often do
  clients ask for more than the maximum", which is among the more actionable
  things this telemetry could report.
  Date/Author: 2026-08-16, drafting agent.

- Decision: introduce `googletest` 0.14.3 as a backend dev-dependency.
  Rationale: the requester asked for googletest assertions, was shown that
  `docs/execplans/backend-5-2-2-job-structs-for-generate-route-and-
  enrichment.md:1277` records the crate being added and then deliberately
  removed in the immediately preceding roadmap item, and reaffirmed the
  choice with that evidence in hand. Under `AGENTS.md` a reaffirmed request
  is a requirement. The crate is Apache-2.0 with a minimum supported Rust
  version of 1.85.0, compatible with the pinned 1.90.0 toolchain. To stop
  the decision oscillating a third time, EP-M6 records it as the standing
  convention in `docs/developers-guide.md`.
  Date/Author: 2026-08-16, requester, recorded by drafting agent.

- Decision: enable the `metrics` feature in the production container image
  and close the public `/metrics` path at the ingress.
  Rationale: the requester chose to enable the feature so that pagination
  telemetry is not dead code in production. Subsequent inspection showed the
  ingress routes `/` to the same port `actix-web-prom` serves `/metrics`
  from, so enabling the feature without an ingress change would publish
  unauthenticated operational data. The ingress exclusion plus an in-cluster
  ServiceMonitor is the smallest change that delivers the requested
  behaviour safely. **This specific mitigation is an assumption pending
  confirmation; see `Risks`.**
  Date/Author: 2026-08-16, requester (feature) and drafting agent
  (mitigation).

- Decision: do not extend `tools/architecture-lint` in this plan, and do not
  touch `RouteMetrics`.
  Rationale: `competing-metrics-patterns` EP-M4 removes the dead
  `RouteMetrics` port and EP-M5 extends the lint to forbid `prometheus`
  imports outside the sanctioned modules. Duplicating either would conflict.
  This plan depends on both and asserts neither.
  Date/Author: 2026-08-16, drafting agent.

## Outcomes & retrospective

To be completed at milestone boundaries and at completion.

## Context and orientation

The backend is an Actix Web application under `backend/`, laid out
hexagonally. `backend/src/domain/` holds pure business logic, with port
traits — interfaces the domain requires — one per file under
`backend/src/domain/ports/` and re-exported from its `mod.rs`.
`backend/src/inbound/http/` holds driving adapters, that is, HTTP handlers.
`backend/src/outbound/` holds driven adapters, with Prometheus
implementations under `backend/src/outbound/metrics/`.
`backend/src/server/mod.rs` is the composition root, selecting real or
`NoOp` implementations from configuration and the `metrics` cargo feature.

Keyset pagination, sometimes called cursor pagination, returns a page of rows
plus an opaque token identifying the position of the last row in a stable
sort order. The client sends that token back to fetch the adjacent page.
There is no page number, which is why "how deep has this client traversed"
is not directly observable from a single request.

The shared primitives live in `backend/crates/pagination`, a
transport-neutral and persistence-neutral leaf crate providing `Cursor<Key>`,
`Direction`, `PageParams`, and the `Paginated<T>` envelope, with a default
page size of 20 and a maximum of 100.

`GET /api/v1/users` is the only endpoint on that crate today. Its request
path is worth following, because every value this plan records is already in
scope somewhere along it.

- `backend/src/inbound/http/users.rs:219-230` — `list_users` obtains the
  session user, calls `parse_users_page_params`, calls the `UsersQuery`
  port, and calls `build_users_page_response`.
- `backend/src/inbound/http/users_pagination.rs:128-147` —
  `parse_users_page_params` parses the raw query, decodes the cursor, and
  returns `(PageParams, ListUsersPageRequest, UsersPageDirection)`. Whether
  `limit` was supplied is visible here as `params.limit.is_none()`.
- `backend/src/inbound/http/users_pagination.rs:89-97` —
  `UsersPageDirection` is already `First | Next | Prev`.
- `backend/src/inbound/http/users_pagination.rs:192-211` —
  `build_users_page_response` holds `has_more`, the row vector, the
  effective limit, and both boundary cursors simultaneously. This is the
  single point where every field of an observation is available.
- `backend/src/inbound/http/users_pagination.rs:233-248` —
  `boundary_cursor` decides which links are emitted, from the tuple of
  page direction, cursor direction, and `has_more`.

The admin provenance endpoint at
`backend/src/inbound/http/admin_enrichment.rs` uses a separate legacy
`before` cursor scheme and is out of scope; roadmap 4.2.4 to 4.2.8 migrate
it, and the port defined here is shaped so that it can adopt this telemetry
without change.

Terminology used below. A *port* is a trait owned by the domain describing
something the domain needs. An *adapter* implements a port against real
infrastructure. *Cardinality* is the number of distinct time series a metric
produces, which is the product of its label value counts. A *terminal* page
is one with no further page in the direction of travel.

## Conformance basis

No Terms of Reference document exists. The upstream artefacts are as
follows.

- `docs/backend-roadmap.md:246-247` — roadmap item 4.2.3, the requirement
  discharged here. Items 4.2.4 to 4.2.8 are the scheduled second consumer.
- `docs/keyset-pagination-design.md` — the pagination crate design,
  including the ordering contract and the explicit statement at lines
  290-310 that cursors are unsigned and client-tamperable.
- `docs/wildside-backend-architecture.md` — the hexagonal guardrails at
  lines 118-300, the shared pagination crate narrative at lines 2001-2066
  where roadmap 4.2.2 recorded its metric, and the custom metric catalogue
  at lines 2565-2593.
- `docs/execplans/competing-metrics-patterns.md` — **the authoritative
  metrics pattern**. This plan consumes its port shape, its `NoOp`
  convention, its composition-root injection, and its deletion of
  `backend/src/observability/`.
- `docs/execplans/backend-4-2-1-replace-users-offset-pagination-with-new-crate.md`
  and `docs/execplans/backend-4-2-2-surface-pagination-aware-errors.md` —
  the immediately preceding items on this endpoint.
- `AGENTS.md` and `docs/documentation-style-guide.md` — style, gates, and
  documentation obligations.

Trace links:

```plaintext
ROADMAP-4.2.3 (page size)        -> EP-M1 -> EP-M4 -> V-COUNTS-MATCH
ROADMAP-4.2.3 (cursor direction) -> EP-M3 -> EP-M4 -> V-OUTCOME-DERIVED
ROADMAP-4.2.3 (traversal counts) -> EP-M2 -> EP-M4 -> V-COUNTS-MATCH
CONS-PATTERN-A (approved idiom)  -> EP-M1 -> EP-M2 -> V-LOG-ALWAYS
ARCH-PORTS-HOME                  -> EP-M1 -> backend/src/domain/ports/pagination_page_metrics.rs
EP-WIRED (no dead adapters)      -> EP-M5 -> V-WIRED
EP-PROD (telemetry in the image) -> EP-M6 -> V-IMAGE-SCRAPE
```

## Verification plan

Axioms, not verified here. The `prometheus` crate aggregates `IntCounterVec`
increments correctly and renders them through `Registry::gather`.
`actix-web-prom` serves a registry at `/metrics`. `tracing` delivers
structured events to installed subscribers. Diesel and PostgreSQL return
rows in the requested order. Repository-owned logic is verified against a
real `prometheus::Registry` and a real embedded PostgreSQL instance, never a
mock of either.

The change introduces four non-trivial invariants and one behavioural
contract. Each is stated below with its method, domain, artefact, evidence,
and non-vacuity argument.

- Obligation: V-LABEL-CLOSED — every label tuple this adapter can emit is
  drawn from a finite set of at most 21 combinations per endpoint, namely
  18 for `pages_total` (3 traversals × 2 outcomes × 3 limit sources) and 3
  for `rows_returned_total` (3 traversals).
  Method: exhaustive parameterized test over the full Cartesian product,
  asserting each combination produces a distinct, gatherable series, plus
  the compile-time closure that every label value originates from a
  fieldless enum with `const fn as_label`.
  Rationale: the domain is finite and small, so enumeration is complete;
  property generation would add nothing a full sweep does not already give.
  Domain: all 18 and all 3 combinations.
  Artefact:
  `backend/src/outbound/metrics/prometheus_pagination_pages.rs` tests.
  Evidence: red — the test file fails to compile before the adapter exists;
  green — `cargo nextest run -p backend prometheus_pagination` passes and
  `Registry::gather` reports exactly 21 series after the sweep.
  Non-vacuity: a seeded mutation making two `as_label` arms return the same
  string collapses two series into one and the gathered count drops to 20,
  failing the test for the intended reason.

- Obligation: V-LABEL-CORRECT — each gathered series carries the label
  *names* paired with the intended *values*, not merely a distinct
  combination of values.
  Method: assert over `Registry::gather`, reading `label_pair()` on a
  representative sample and checking name-to-value pairing explicitly, for
  example that the pair named `traversal` holds `"prev"` and the pair named
  `outcome` holds `"terminal"`.
  Rationale: V-LABEL-CLOSED counts series and would pass unchanged if the
  four label values were transposed, because a permutation of values across
  a fixed set of names yields exactly the same number of distinct series.
  Every dashboard and alert would then be silently wrong. A count-based
  assertion cannot detect this, so a separate pairing assertion is required.
  This is the vacuity hole V-LABEL-CLOSED cannot close by itself.
  Domain: one series per metric family, chosen so that all four label values
  differ from one another, so that no transposition is a fixed point.
  Artefact:
  `backend/src/outbound/metrics/prometheus_pagination_pages.rs` tests.
  Evidence: red — fails before the adapter exists; green — the pairing
  assertions pass.
  Non-vacuity: a seeded mutation swapping the `traversal` and `outcome`
  arguments in the `with_label_values` call must fail this test while
  leaving V-LABEL-CLOSED green. Both halves of that statement must be
  recorded, since the point of the obligation is the gap between them.

- Obligation: V-RECORD-NOT-INCREMENT — the inbound adapter reaches the port
  through `record`, so the structured log is emitted on every delivered
  page, and passes an observation whose fields match the response actually
  returned.
  Method: unit test with a `mockall` mock of the port, asserting `record` is
  called exactly once per delivered page with the expected observation, and
  that `increment` is never called directly by inbound code.
  Rationale: the provided-method idiom is only load-bearing if call sites
  use it; a caller that reaches for `increment` compiles, passes every
  counter assertion, and silently loses logging in all builds. No type check
  catches this.
  Domain: one call per traversal variant.
  Artefact: `backend/src/inbound/http/users_pagination/telemetry.rs` tests.
  Evidence: red — fails before emission exists; green — the expectation is
  satisfied.
  Non-vacuity: rewriting the call site to use `increment` must fail this
  test while leaving V-COUNTS-MATCH green, because the counters still move.

- Obligation: V-OUTCOME-DERIVED — `outcome` is `Terminal` exactly when the
  boundary link for the direction of travel is absent, and `Continuing`
  otherwise. It is *not* `!has_more`.
  Method: exhaustive parameterized test over the same tuple space that
  `boundary_cursor` dispatches on — page direction × cursor direction ×
  `has_more` — asserting the derived outcome against the emitted links.
  Rationale: the domain is 3 × 2 × 2 = 12 cases. Exhaustive enumeration over
  a finite domain is a complete argument, so a deductive proof would restate
  rather than strengthen it. This is why no `verus` obligation is raised.
  Domain: all 12 combinations, including the two where `has_more` is true
  but the direction-specific link is absent.
  Artefact: `backend/src/inbound/http/users_pagination/telemetry.rs` tests.
  Evidence: red — written before the derivation exists; green — all 12 pass.
  Non-vacuity: a seeded fault deriving `outcome` from `!has_more` must fail
  on the `(First, Prev)` case, where `boundary_cursor` emits no previous
  link regardless of `has_more`. That case is the negative control and must
  be named in the test.

- Obligation: V-OBS-SATURATES — a constructed observation always satisfies
  `returned_rows <= page_limit`, and `returned_rows < page_limit` implies
  `!has_more`. Violations indicate a server defect and must saturate, never
  panic, because a telemetry defect must not fail a served request.
  Method: property test over arbitrary `usize` pairs and both `has_more`
  values, asserting both invariants hold of the constructed value.
  Rationale: the input domain is unbounded, and the interesting behaviour is
  at and beyond the boundary, which enumeration cannot cover.
  Domain: `page_limit` in `1..=100`, `returned_rows` in `0..=usize::MAX`,
  `has_more` both values.
  Artefact: `backend/src/domain/ports/pagination_page_metrics.rs` tests.
  Evidence: red — fails before the saturating constructor exists; green —
  passes with a recorded case count.
  Non-vacuity: the test must classify and assert that both the in-range and
  the out-of-range generation classes are reached, so a generator that only
  ever produced valid pairs is itself a failure. A mutation removing the
  `.min()` clamp must be rejected by the first out-of-range case.

- Obligation: V-LOG-ALWAYS — the provided `record` method emits the
  structured event with correct field values for every implementation,
  including `NoOp`, and therefore in builds without the `metrics` feature.
  Method: unit test capturing `tracing` output with the already-present
  `tracing-test` dev-dependency, asserting on the `endpoint`, `traversal`,
  `outcome`, `limit_source`, and `rows` field values.
  Rationale: the invariant concerns an observable side effect that no type
  check can see.
  Domain: one call per traversal variant against `NoOpPaginationPageMetrics`.
  Artefact: `backend/src/domain/ports/pagination_page_metrics.rs` tests.
  Evidence: red — fails to compile against the absent port; green — passes.
  Non-vacuity: asserting field *values* rather than mere event presence
  means both a silent implementation and a wrong-label implementation fail.

- Obligation: V-COUNTS-MATCH — traversing N pages forward produces exactly N
  increments of `pages_total` with `traversal` in `{first, next}` and the
  correct `rows_returned_total` sum; a subsequent backward step produces one
  increment with `traversal="prev"`.
  Method: behavioural tests with `rstest-bdd`, extending
  `backend/tests/features/users_list_pagination.feature` against embedded
  PostgreSQL with the existing five-user ordered seed.
  Rationale: this is the end-to-end contract the roadmap item names, and it
  crosses the HTTP boundary, the port, and the adapter together.
  Domain: full forward traversal to exhaustion, a next-then-previous
  traversal, and a default-limit versus explicit-limit pair.
  Artefact: `backend/tests/features/users_list_pagination.feature` and
  `backend/tests/users_list_pagination_bdd/`.
  Evidence: red — the new scenarios fail because no counter moves; green —
  gathered counter values match the scenario table.
  Non-vacuity: the scenarios assert exact counter values, not merely
  non-zero ones, so an implementation that increments on every request
  regardless of traversal fails the `traversal="prev"` case.

- Obligation: V-WIRED — a server constructed through the ordinary
  composition path with metrics enabled actually exposes both metric
  families at `/metrics`.
  Method: extend the existing real-server test at
  `backend/src/tests.rs:151-174`, which already starts a server with metrics
  attached, to issue `GET /metrics` and assert both family names appear;
  and extend `backend/tests/state_builders_composition_unit.rs` and
  `backend/tests/startup_mode_composition_bdd.rs` to assert the port is
  bound to the Prometheus implementation rather than the `NoOp` when a
  registry is present.
  Rationale: this is the only obligation that distinguishes "the adapter
  works" from "the adapter runs". Two existing adapters pass thorough unit
  tests while never being constructed in production.
  Domain: metrics-enabled composition with and without a database pool.
  Artefact: `backend/src/tests.rs`, the two composition suites.
  Evidence: red — the scrape assertion fails before EP-M5 wires anything;
  green — both family names appear in the response body.
  Non-vacuity: deleting the wiring line in the composition root must make
  this fail while every unit test still passes. That is precisely the
  historical failure this obligation exists to catch, so the mutation must
  be performed and recorded.

- Obligation: V-FEATURE-OFF — the crate builds and its tests pass without
  the `metrics` feature, and page telemetry still logs.
  Method: `cargo check -p backend --no-default-features`, plus V-LOG-ALWAYS,
  which is not feature-gated.
  Rationale: feature-off behaviour is encoded as `NoOp` selection at
  composition and must be proven by compiling both configurations.
  Non-vacuity: removing the `NoOp` arm from the composition root fails the
  feature-off compile.

- Obligation: V-IMAGE-SCRAPE — the artefact actually deployed serves the
  metrics, and does not serve them publicly.
  Method: a continuous-integration job that builds
  `deploy/docker/backend.Dockerfile`, runs the container, and asserts
  `GET /metrics` returns 200 containing `wildside_pagination_pages_total`;
  plus a `helm template` assertion that the rendered ingress does not route
  `/metrics`.
  Rationale: every other obligation is satisfied by `cargo test`, which has
  been green on a configuration that never shipped. Gating on the artefact
  is the only check that distinguishes them.
  Non-vacuity: reverting the Dockerfile `--features metrics` change must
  fail the scrape assertion; removing the ingress exclusion must fail the
  template assertion.

No further non-trivial invariants arise. The change adds no concurrency, no
ordering constraint, no persisted state, and no wire format. `kani` is not
used: there is no unsafe code, no bounded state machine, and no arithmetic
whose overflow behaviour is in question once depth is excluded — and three
prior execplans declined it on the same grounds. `verus` is not used: the
only contractual logic introduced is V-OUTCOME-DERIVED, whose domain is 12
cases, and exhaustive enumeration discharges it completely rather than
approximately, so a proof would restate the enumeration rather than
strengthen it.

## Plan of work

Stage A, complete: reconnaissance, six-lens design review, and this
document.

Stages B and C run per milestone, red tests before production code
throughout. Stage D is the documentation and deployment milestone.

**EP-M0 — make room.** Split
`backend/src/server/state_builders.rs` (383 of 400 lines) along a cohesive
seam so the wiring in EP-M5 has room. No behaviour change; this is a
separate atomic refactor committed on its own, per `AGENTS.md`. Gates must
be green before and after.

**EP-M1 — the port.** Create
`backend/src/domain/ports/pagination_page_metrics.rs` with the label enums,
the observation type and its saturating constructor, the trait, and the
`NoOp` implementation. Re-export from `backend/src/domain/ports/mod.rs`.
Red: V-LOG-ALWAYS and V-OBS-SATURATES first. No caller changes at this
plateau.

**EP-M2 — the adapter.** Create
`backend/src/outbound/metrics/prometheus_pagination_pages.rs` mirroring the
shape of `prometheus_idempotency.rs`: `new(registry: &Registry) ->
Result<Self, prometheus::Error>` constructing both `IntCounterVec`s and
registering them. Resolve all 21 label children in `new` and index by
discriminant, so the hot path is two `fetch_add` operations with no lock
acquisition. Declare it in `backend/src/outbound/metrics/mod.rs` behind the
same feature gating as its siblings. Red: V-LABEL-CLOSED.

**EP-M3 — one traversal type.** Move `PageTraversal` into the domain port
module and delete `UsersPageDirection` from
`backend/src/inbound/http/users_pagination.rs`, updating its uses and the
two doctests that mention it. This is a mechanical rename plus a deletion
and should be its own commit.

**EP-M4 — emission.** Create
`backend/src/inbound/http/users_pagination/telemetry.rs` holding the
observation construction and the outcome derivation, keeping the parent
module under the line cap. Change `build_users_page_response` to take a
parameter object rather than a fifth positional argument, since it already
sits exactly at `too-many-arguments-threshold = 4`. Derive `outcome` from
the emitted boundary link for the direction of travel, and `limit_source`
from the raw query value before normalization. Red: V-OUTCOME-DERIVED, then
the V-COUNTS-MATCH scenarios.

**EP-M5 — wiring and policy.** Select
`PrometheusPaginationPageMetrics::new(&prom.registry)` when metrics are
enabled and `NoOpPaginationPageMetrics` otherwise, in the same composition
block as the existing idempotency selection. Carry the handle as
`Arc<dyn PaginationPageMetrics>` on `HttpStateExtraPorts`, which has a
`Default` implementation, so none of the eight `HttpStatePorts` construction
sites change. State the registration-failure policy explicitly in code
comments and follow the fatal branch. Red: V-WIRED, including the deletion
mutation.

**EP-M6 — production and documentation.** Add `--features metrics` to
`deploy/docker/backend.Dockerfile`, add the ingress exclusion and a
ServiceMonitor template, add the `googletest` dev-dependency, and update
every document listed under `Documentation obligations`. Tick roadmap item
4.2.3.

Delegate mechanical documentation edits to `scribe`. Run gates through
`scrutineer` after each milestone.

### Documentation obligations

- `docs/wildside-backend-architecture.md` lines 2001-2066 — add a 4.2.3
  paragraph continuing the pagination crate narrative, matching where
  roadmap 4.2.2 recorded its metric. Name the port, the adapter, the metric
  names, the label sets, and the feature gate.
- `docs/wildside-backend-architecture.md` lines 2565-2593 — add the two new
  metrics to the custom metric catalogue, and backfill the four already
  missing from it: `wildside_pagination_errors_total`,
  `wildside_idempotency_requests_total`, `route_queue_enqueue_total`, and
  `route_queue_enqueue_latency_seconds`. The catalogue currently presents
  itself as canonical while omitting them.
- `docs/developers-guide.md` — extend the "Metrics conventions" subsection
  created by `competing-metrics-patterns` with the closed-label-enum rule,
  the derive-in-the-query-layer preference over histograms for bounded small
  integers, and the `googletest` convention including the requirement that
  `#[gtest]` precede `#[rstest]`, since the reverse order registers the test
  twice.
- `docs/keyset-pagination-design.md` — record that traversal depth is
  deliberately *not* carried in the cursor, with the reasons, so the
  question is not reopened without new evidence.
- `docs/users-guide.md` — add a short operator note that `/metrics` is
  served in the shipped image and is not routed publicly. Client-facing
  pagination behaviour is unchanged, so the existing section needs no edit.
- `docs/repository-structure.md:608` — correct the ServiceMonitor claim so
  it matches what EP-M6 actually builds.
- `docs/contents.md` — index this ExecPlan, and add the missing entries for
  `keyset-pagination-design.md` and `developers-guide.md`.
- `docs/backend-roadmap.md:246-247` — mark 4.2.3 done on completion.

No architectural decision record is required. The one decision that would
have warranted an ADR — changing the opaque cursor wire format — was
rejected, and the metrics pattern itself is settled by
`competing-metrics-patterns`. Conventions belong in the developers' guide.

## Milestones and plateaus

- EP-M0 — `state_builders.rs` split; behaviour identical.
  Requirements: none discharged; unblocks EP-M5.
  Acceptance: full gates green; no diff in behaviour.
  Conformance: no interface, dependency, or format change.
  Recovery: revert the single commit.
  Remaining: all telemetry work.
  Compatibility: none required; application-internal.

- EP-M1 — port, enums, observation, and `NoOp` exist and are tested; no
  caller changed.
  Requirements: ROADMAP-4.2.3 partially; CONS-PATTERN-A.
  Acceptance: V-LOG-ALWAYS and V-OBS-SATURATES discharged.
  Conformance: port lives in `domain::ports`; no `prometheus` import in
  domain.
  Recovery: revert; the two new files are additive.
  Remaining: nothing records anything yet.
  Compatibility: none required; pre-1.0 and application-internal.

- EP-M2 — Prometheus adapter exists and is tested against a fresh registry;
  still uncalled.
  Acceptance: V-LABEL-CLOSED and V-LABEL-CORRECT discharged, 21 series
  confirmed, and the label-transposition mutation recorded as failing
  V-LABEL-CORRECT while leaving V-LABEL-CLOSED green.
  Conformance: adapter confined to `outbound::metrics`.
  Recovery: revert.
  Remaining: wiring.

- EP-M3 — one traversal enum in the repository.
  Acceptance: `rg UsersPageDirection backend/` returns nothing; gates green.
  Conformance: abstraction sweep satisfied.
  Recovery: revert.

- EP-M4 — the users endpoint records an observation per delivered page.
  Acceptance: V-OUTCOME-DERIVED, V-RECORD-NOT-INCREMENT, and V-COUNTS-MATCH
  discharged, including the `increment`-at-call-site mutation.
  Conformance: `list_users` still imports no outbound module; response
  envelope byte-identical.
  Recovery: revert; the port and adapter remain harmlessly unused.
  Remaining: the recorder is still the `NoOp` in production.

- EP-M5 — telemetry is live in a metrics-enabled build.
  Acceptance: V-WIRED and V-FEATURE-OFF discharged, including the recorded
  deletion mutation.
  Conformance: registration-failure policy stated; no persisted or wire
  format change.
  Recovery: revert this commit to return to the EP-M4 plateau.

- EP-M6 — telemetry is live in the shipped image and documented.
  Acceptance: V-IMAGE-SCRAPE discharged; `make markdownlint` and
  `make nixie` green; roadmap ticked.
  Conformance: the ingress change is a deployment-surface change and must be
  confirmed at approval per `Risks`.
  Recovery: revert; the container returns to a default-feature build.

Each milestone is committed separately with gates run beforehand. No
compatibility machinery is introduced at any milestone: every interface
touched is application-internal and pre-1.0, and no deployed peer or
persisted format depends on it.

## Concrete steps

All commands run from the repository root. Prefer delegating gate runs to
`scrutineer`; when running directly, use `tee`.

```bash
make check-fmt 2>&1 | tee "/tmp/fmt-wildside-$(git branch --show-current).out"
make lint 2>&1 | tee "/tmp/lint-wildside-$(git branch --show-current).out"
make test 2>&1 | tee "/tmp/test-wildside-$(git branch --show-current).out"
make markdownlint 2>&1 | tee "/tmp/mdlint-wildside-$(git branch --show-current).out"
make nixie 2>&1 | tee "/tmp/nixie-wildside-$(git branch --show-current).out"
```

`make check-fmt` and `make lint` can exit 0 despite sub-check failures. Read
the log, not the exit code. The local Whitaker Dylint suite is ahead of the
pin used in continuous integration, so `make lint` may be red on a clean
tree; take a baseline before attributing failures to this work.

Feature-off check for V-FEATURE-OFF:

```bash
cargo check -p backend --no-default-features 2>&1 | \
  tee "/tmp/check-nofeat-wildside-$(git branch --show-current).out"
```

Focused runs during red and green stages:

```bash
cargo nextest run -p backend prometheus_pagination
cargo nextest run -p backend pagination_page_metrics
cargo nextest run -p backend --test users_list_pagination_bdd
```

Expected red-stage transcript shape at EP-M2, before the adapter exists:

```plaintext
error[E0432]: unresolved import `crate::outbound::metrics::PrometheusPaginationPageMetrics`
```

Expected shape of the V-WIRED scrape assertion once green:

```plaintext
wildside_pagination_pages_total{endpoint="users",limit_source="default",outcome="terminal",traversal="first"} 1
wildside_pagination_rows_returned_total{endpoint="users",traversal="first"} 5
```

## Validation and acceptance

Red, green, refactor applies to every milestone. Red: the new test compiles
against a missing type, or asserts behaviour the code does not yet have; run
the focused command and record the failure reason in `Progress`. Green: the
smallest implementation that passes; rerun the focused command. Refactor:
tidy, then run the full gates through `scrutineer`.

Behavioural acceptance, phrased as observable behaviour. Start the server
with `--features metrics` and a seeded database. Request
`GET /api/v1/users?limit=2` and observe a 200 with two rows and a `next`
link. Scrape `/metrics` and observe `wildside_pagination_pages_total` with
`traversal="first"`, `outcome="continuing"`, `limit_source="explicit"` at
value 1, and `wildside_pagination_rows_returned_total` with
`traversal="first"` at value 2. Follow the `next` link twice more to
exhaust the five seeded users, and observe the counter reach 3 across the
`first` and `next` traversals with the final page carrying
`outcome="terminal"`. Follow the returned `prev` link and observe one
increment at `traversal="prev"`.

Done means every verification obligation is discharged with evidence
recorded in this document; `make check-fmt`, `make lint` against baseline,
`make test`, `make markdownlint`, and `make nixie` are green; the V-WIRED
deletion mutation has been performed and recorded; the container scrape
check passes; every document listed under `Documentation obligations` is
updated; roadmap 4.2.3 is ticked; and seven commits exist, one per
milestone, with imperative, wrapped messages.

Quality criteria:

- Tests: all suites green under `cargo nextest run --workspace
  --all-targets --all-features`, plus the feature-off compile.
- Verification: V-LABEL-CLOSED, V-LABEL-CORRECT, V-OUTCOME-DERIVED,
  V-OBS-SATURATES, V-LOG-ALWAYS, V-RECORD-NOT-INCREMENT, V-COUNTS-MATCH,
  V-WIRED, V-FEATURE-OFF, and V-IMAGE-SCRAPE all discharged with the stated
  non-vacuity evidence. Three of these are guarded by a mutation that must
  fail one obligation while leaving a sibling green — V-LABEL-CORRECT
  against V-LABEL-CLOSED, V-RECORD-NOT-INCREMENT against V-COUNTS-MATCH, and
  the wiring deletion for V-WIRED. Each mutation must be performed and both
  halves of its outcome recorded, not merely asserted.
- Lint and typecheck: `make lint` green against a pre-change baseline,
  including both Whitaker manifest passes under `-D warnings`.
- Performance: no benchmark threshold. The recording path must be two
  atomic increments with pre-resolved label children and no allocation; this
  is a structural requirement on the adapter, confirmed by review rather
  than measurement.
- Security: `/metrics` must not be routable through the ingress. Confirmed
  by the `helm template` assertion in V-IMAGE-SCRAPE.

## Idempotence and recovery

Every milestone is a self-contained commit, and `git revert` of any single
milestone restores the previous plateau. There are no data migrations and no
persisted state. EP-M6 is the only milestone touching deployment; reverting
it returns the container to a default-feature build, which is the current
production behaviour, so the rollback is to a known-good state rather than
to an untested one. Gate logs under `/tmp` are scratch and may be deleted.

## Artefacts and notes

Reconnaissance evidence gathered 2026-08-16.

- Approved pattern: `docs/execplans/competing-metrics-patterns.md`, plan of
  work EP-M1 through EP-M5, establishing the port, `NoOp`, adapter, and
  composition-root injection, and deleting
  `backend/src/observability/pagination_errors.rs`.
- Existing adapters to mirror:
  `backend/src/outbound/metrics/prometheus_idempotency.rs:36`, and the
  bucket and label discipline in `prometheus_route_queue.rs:21-33`.
- Existing sync, infallible port to mirror:
  `backend/src/domain/ports/route_queue_metrics.rs:29-40`.
- Unwired adapters demonstrating the failure V-WIRED prevents:
  `PrometheusRouteQueueMetrics` and `PrometheusEnrichmentJobMetrics` are
  constructed only under `#[cfg(test)]`.
- Line-count headroom: `backend/src/server/state_builders.rs` 383,
  `backend/src/inbound/http/users_pagination.rs` 316, against a 400 cap.
- `clippy.toml`: `too-many-arguments-threshold = 4`,
  `cognitive-complexity-threshold = 9`, `too-many-lines-threshold = 70`.
- `.config/nextest.toml:54` already places `users_list_pagination_bdd` in
  the `pg-embed` group with a 300-second timeout, so no runner change is
  required.

## Interfaces and dependencies

One new dev-dependency: `googletest = "0.14"` under
`[dev-dependencies]` in `backend/Cargo.toml`. No new runtime dependency.

In `backend/src/domain/ports/pagination_page_metrics.rs`, define:

```rust
/// Endpoint identity for pagination telemetry.
///
/// Values become Prometheus label values, so the set is closed at compile
/// time and must stay small.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PaginatedEndpoint {
    Users,
}

/// How the caller reached this page.
///
/// Replaces `inbound::http::users_pagination::UsersPageDirection`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageTraversal {
    First,
    Next,
    Prev,
}

/// Whether a further page exists in the direction of travel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageOutcome {
    Continuing,
    Terminal,
}

/// Provenance of the effective page size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitSource {
    Default,
    Explicit,
    Clamped,
}

/// One delivered page. `Copy`; pass by value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaginationPageObservation {
    pub endpoint: PaginatedEndpoint,
    pub traversal: PageTraversal,
    pub outcome: PageOutcome,
    pub limit_source: LimitSource,
    pub page_limit: NonZeroUsize,
    pub returned_rows: usize,
}

/// Records delivered pages.
///
/// Call [`PaginationPageMetrics::record`], never `increment`: `record` is
/// what emits the structured log, and it delegates. Calling `increment`
/// directly compiles and moves the counters while silently dropping the log
/// in every build, including builds without the `metrics` feature.
#[cfg_attr(test, mockall::automock)]
pub trait PaginationPageMetrics: Send + Sync {
    /// Increment the page counters for `observation`.
    ///
    /// Implementation hook. Prefer `record` at call sites.
    fn increment(&self, observation: PaginationPageObservation);

    /// Emit the structured log, then increment.
    fn record(&self, observation: PaginationPageObservation) {
        // tracing::debug! with endpoint, traversal, outcome, limit_source,
        // page_limit, rows, and trace_id.
        self.increment(observation);
    }
}

pub struct NoOpPaginationPageMetrics;
```

`#[cfg_attr(test, mockall::automock)]` follows the prevailing port
convention — 26 of the ports under `backend/src/domain/ports/` carry it,
including the closest analogue, `enrichment_job_metrics.rs` — and supplies
the mock that V-RECORD-NOT-INCREMENT needs. `mockall` is already a backend
dev-dependency at version 0.15.

Each label enum gains a `const fn as_label(self) -> &'static str`.
`PaginationPageObservation::new` saturates rather than panics, discharging
V-OBS-SATURATES.

In `backend/src/outbound/metrics/prometheus_pagination_pages.rs`, behind the
`metrics` feature:

```rust
pub struct PrometheusPaginationPageMetrics {
    pages_total: IntCounterVec,
    rows_returned_total: IntCounterVec,
}

impl PrometheusPaginationPageMetrics {
    pub fn new(registry: &prometheus::Registry) -> Result<Self, prometheus::Error>;
}
```

Metric contract, which dashboards treat as a wire format:

- `wildside_pagination_pages_total`, labels
  `["endpoint", "traversal", "outcome", "limit_source"]`, 18 series per
  endpoint.
- `wildside_pagination_rows_returned_total`, labels
  `["endpoint", "traversal"]`, 3 series per endpoint.

Label order is load-bearing. `IntCounterVec::with_label_values` takes a
`&[&str]` matched positionally against the names given to `Opts`, so the
call must pass exactly `[endpoint, traversal, outcome, limit_source]` in
that order. The approved plan makes the same point for the two-label error
counter; with four labels the hazard is larger, because most transpositions
still yield well-formed, distinct, plausible-looking series. Declare the
name array once as a private constant and build the value array in the same
function, so the two cannot drift. V-LABEL-CORRECT is the gate.

Derived queries, which is where page size and traversal depth are answered:

```plaintext
mean delivered page size =
  rate(wildside_pagination_rows_returned_total[5m])
  / rate(wildside_pagination_pages_total[5m])

mean forward traversal depth =
  rate(wildside_pagination_pages_total{traversal="next"}[1h])
  / rate(wildside_pagination_pages_total{traversal="first"}[1h])

traversal exhaustion rate =
  rate(wildside_pagination_pages_total{outcome="terminal"}[1h])
  / rate(wildside_pagination_pages_total{traversal="first"}[1h])
```

In `backend/src/server/mod.rs`, composition selects the implementation and
the handle travels as `Arc<dyn PaginationPageMetrics>` on
`HttpStateExtraPorts`.

Deleted by this plan: `UsersPageDirection` in
`backend/src/inbound/http/users_pagination.rs`.

Explicitly not touched: `backend/crates/pagination` in any form,
`RouteMetrics` (removed by `competing-metrics-patterns` EP-M4), and
`tools/architecture-lint` (extended by that plan's EP-M5).

## Revision note

2026-08-16, second revision. Re-audited this plan element by element against
`docs/execplans/competing-metrics-patterns.md` at commit `750c3d8`, which is
unchanged since the first alignment. The port, `NoOp`, adapter,
`&Registry` construction, composition-root injection, absence of global
state, synchronous infallible signature, and the required-`increment` plus
provided-`record` idiom all already conformed. The audit found three gaps,
now closed.

The first was a vacuity hole in this plan's own verification. V-LABEL-CLOSED
asserts the number of distinct series, but a transposition of label values
across a fixed set of label names produces exactly the same count, so every
dashboard could be silently wrong with that obligation green. V-LABEL-CORRECT
now asserts name-to-value pairing, and is guarded by a mutation that must
fail it while leaving V-LABEL-CLOSED passing. The approved plan makes the
same argument-order point for its two-label counter; the hazard is larger
here because four labels admit more plausible-looking transpositions.

The second was that nothing pinned call sites to `record` rather than
`increment`. The provided-method idiom is the mechanism by which logging
survives in builds without the `metrics` feature, and a caller reaching for
`increment` would compile, pass every counter assertion, and silently lose
the log. V-RECORD-NOT-INCREMENT closes this, with a mutation that must fail
it while leaving V-COUNTS-MATCH green.

The third was the missing `#[cfg_attr(test, mockall::automock)]`. Twenty-six
ports carry it, including the closest analogue `enrichment_job_metrics.rs`,
and it supplies the mock V-RECORD-NOT-INCREMENT needs.

Effect on remaining work: EP-M2 and EP-M4 each gain one obligation and one
recorded mutation. No milestone boundary, interface, or metric contract
changed, and the scope tolerance is unaffected.
