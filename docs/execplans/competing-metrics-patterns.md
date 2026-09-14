# Consolidate the backend's two competing metrics patterns

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, `Outcomes & Retrospective`, `Conformance Basis`, and
`Verification Plan` must be kept up to date as work proceeds.

Status: DRAFT

## Purpose / big picture

The backend records application metrics in two structurally incompatible ways.

Pattern A is hexagonal: the domain defines a metrics port (a trait in
`backend/src/domain/ports/`), a `NoOp*` implementation serves tests and
metrics-disabled builds, and a `Prometheus*` adapter in
`backend/src/outbound/metrics/` implements the trait against an explicitly
injected `prometheus::Registry`. `IdempotencyMetrics`, `RouteQueueMetrics`,
and `EnrichmentJobMetrics` all follow this shape, and the implementation is
selected at server-composition time in `backend/src/server/mod.rs`.

Pattern B is a module-level global: `backend/src/observability/
pagination_errors.rs` holds a `static PAGINATION_ERRORS_TOTAL:
OnceLock<IntCounterVec>`, populated at process start by
`register_pagination_error_metrics(&Registry)` (re-exported at the crate root
and called from `backend/src/main.rs`), and incremented from anywhere via the
bare free function `record_pagination_error`. Feature gating is done by
duplicating function bodies under `#[cfg(feature = "metrics")]` /
`#[cfg(not(...))]` instead of a `NoOp` implementation. The function is called
directly from an inbound HTTP adapter
(`backend/src/inbound/http/users_pagination.rs`) and an outbound persistence
adapter (`backend/src/outbound/persistence/user_persistence_error_mapping.rs`).

After this change, exactly one pattern remains: every metric is recorded
through an injected trait with a `NoOp` and a Prometheus implementation, the
`OnceLock` global and the crate-root registration function are gone, and the
convention is written down in `docs/developers-guide.md`. Observable success:
`rg "OnceLock" backend/src` returns no metrics-related hits, the
`wildside_pagination_errors_total` counter still increments with the same
name and labels for the same events, and all commit gates pass. The plan also
resolves the dead `RouteMetrics` port (defined and re-exported in
`backend/src/domain/ports/route_metrics.rs` but never implemented or called)
by removing it.

## Constraints

- The exported metric name `wildside_pagination_errors_total` and its label
  set `["code", "source"]` must not change; dashboards and alerting treat the
  exposition format as a wire format.
- The structured `tracing::info!` log emitted when a pagination error is
  mapped must continue to be emitted in all builds, including builds without
  the `metrics` cargo feature.
- The backend must continue to compile and pass tests both with and without
  the `metrics` cargo feature (`backend/Cargo.toml` feature
  `metrics = ["dep:actix-web-prom", "dep:prometheus"]`).
- Domain code (`backend/src/domain/`) must not gain any `prometheus` or
  `actix-web-prom` import; the dependency rule of the hexagonal layout holds
  throughout.
- No new external dependencies.
- Do not re-litigate the recorded decision to keep per-feature metrics ports
  separate (see `docs/execplans/backend-phase-1-idempotency-audit-metrics.md`,
  decision log); this plan unifies the *pattern*, not the ports.
- Follow `AGENTS.md`: en-GB-oxendict spelling, files under 400 lines, docs
  updated alongside code, all commit gates run before each commit.

## Tolerances (exception triggers)

- Scope: stop and escalate if the change exceeds 20 files or 600 net lines.
- Interface: the crate-root re-export `register_pagination_error_metrics` in
  `backend/src/lib.rs` is application-internal and will be deleted without a
  compatibility shim; escalate only if an out-of-crate consumer other than
  `backend/src/main.rs` turns up.
- Iterations: if a gate still fails after three fix attempts on the same
  failure, stop and escalate with the log path.
- Ambiguity: if threading the injected handle into
  `diesel_login_service.rs` forces a public constructor signature change that
  ripples beyond `backend/src/server/mod.rs` and test support code, stop and
  present options.

## Risks

- Risk: the persistence error-mapping functions are free functions called
  from several adapters (`diesel_login_service.rs`, the user repository), so
  threading an injected handle may touch more call sites than mapped here.
  Severity: medium. Likelihood: medium.
  Mitigation: milestone EP-M3 starts with `leta refs` sweeps of
  `map_user_persistence_error` and `record_pagination_error`; escalate via
  the scope tolerance if the fan-out exceeds it.
- Risk: `serial_test`-guarded tests depend on the `OnceLock` global's
  process-wide lifetime; removing the global changes test isolation
  assumptions. Severity: low. Likelihood: medium.
  Mitigation: the replacement adapter takes `&Registry` at construction, so
  each test builds a fresh `Registry` and the `#[serial]` guard can be
  dropped.
- Risk: removing `RouteMetrics` conflicts with the architecture document,
  which sketches it as intended design (`docs/wildside-backend-architecture.md`
  around lines 2877–2880). Severity: low. Likelihood: high.
  Mitigation: EP-M4 updates the architecture document in the same commit and
  records the removal in the decision log; the trait is trivially
  re-introducible when a route cache lands.

## Progress

- [x] (2026-08-16) Reconnaissance complete: both patterns characterized with
  file:line evidence; `RouteMetrics` confirmed dead (definition and
  `pub use` only, no implementations or call sites).
- [ ] EP-M1: pagination metrics port and `NoOp` implementation (red tests
  first).
- [ ] EP-M2: Prometheus adapter in `backend/src/outbound/metrics/`.
- [ ] EP-M3: wire injection through inbound and outbound call sites; delete
  the `OnceLock` global, the duplicate `cfg` function bodies, the `lib.rs`
  re-export, and the `main.rs` registration call.
- [ ] EP-M4: remove the dead `RouteMetrics` port; update
  `docs/wildside-backend-architecture.md`.
- [ ] EP-M5: document the single metrics convention in
  `docs/developers-guide.md`; extend `tools/architecture-lint` to forbid
  `prometheus` imports outside the sanctioned modules.

## Surprises & discoveries

- Observation: no repository document names the "two competing metrics
  patterns"; the phrase comes from the task, and the closest documented
  tension is the deliberate proliferation of per-feature ports.
  Evidence: full-tree Markdown sweep for
  `metrics|competing|prometheus|observability|instrumentation|recorder`.
  Impact: this plan is the artefact that records the divergence and its
  resolution.
- Observation: both patterns already share one `prometheus::Registry`, owned
  by `actix_web_prom::PrometheusMetrics` built in `backend/src/main.rs` with
  endpoint `/metrics`.
  Evidence: `backend/src/main.rs` lines 28–31.
  Impact: consolidation changes recording style only; exposition is
  untouched.

## Decision log

- Decision: adopt Pattern A (port + `NoOp` + Prometheus adapter, injected at
  composition time) as the single sanctioned metrics pattern, and migrate
  pagination-error recording to it.
  Rationale: Pattern A already covers three of four metric families, keeps
  the domain free of `prometheus` types, gives per-test registry isolation,
  and avoids the `OnceLock` global's hidden temporal coupling (silently
  recording nothing if registration is forgotten). The alternative —
  blessing the global free-function style — would spread implicit global
  state and duplicate-`cfg` bodies to every future metric.
  Date/Author: 2026-08-16, drafting agent.
- Decision: give the pagination port a provided method that always emits the
  structured `tracing` log and then calls a required `increment` hook,
  instead of splitting logging from metrics at every call site.
  Rationale: preserves the existing log-always, count-when-enabled semantics
  in one place; `NoOpPaginationErrorMetrics` gets logging for free.
  Date/Author: 2026-08-16, drafting agent.
- Decision: the pagination port is synchronous and infallible
  (`fn`, returns `()`), unlike `IdempotencyMetrics` (async, fallible).
  Rationale: its call sites are synchronous error-mapping functions on the
  request path; counter increments cannot meaningfully fail, and a metrics
  failure must never alter request handling. Matches the current behaviour
  of `increment_pagination_error_counter`.
  Date/Author: 2026-08-16, drafting agent.
- Decision: remove `RouteMetrics` and `RouteMetricsError` rather than wire
  them.
  Rationale: pre-1.0, application-internal, zero implementations and zero
  call sites; a dead port invites divergent guesses about its semantics.
  `AGENTS.md` requires sweeping for existing abstractions before adding new
  ones — a phantom abstraction poisons that sweep. The architecture document
  is updated in the same milestone so design and code agree.
  Date/Author: 2026-08-16, drafting agent.

## Outcomes & retrospective

To be completed at milestone boundaries and at completion.

## Context and orientation

The backend is an Actix Web application in `backend/`, laid out hexagonally:

- `backend/src/domain/` — pure business logic; `backend/src/domain/ports/`
  holds the port traits (interfaces the domain requires), one file per port,
  re-exported from `backend/src/domain/ports/mod.rs`.
- `backend/src/inbound/http/` — driving adapters (HTTP handlers). Handlers
  receive shared state as `actix_web::web::Data<HttpState>`.
- `backend/src/outbound/` — driven adapters. `backend/src/outbound/metrics/`
  holds the Prometheus implementations of the metrics ports;
  `backend/src/outbound/persistence/` holds Diesel-backed persistence.
- `backend/src/server/mod.rs` — composition root; selects real or `NoOp`
  implementations based on configuration and the `metrics` cargo feature
  (see `build_route_submission_service`, lines ~98–133, matching on
  `(config.db_pool, config.metrics())`).
- `backend/src/observability/pagination_errors.rs` — the Pattern B module
  this plan dissolves (117 lines: enum `PaginationErrorSource`, free
  functions `record_pagination_error`, `increment_pagination_error_counter`
  ×2, `register_pagination_error_metrics`, one `#[serial]` test).
- `tools/architecture-lint/` — a `syn`-based lint binary that walks
  `backend/src` and rejects forbidden imports per layer
  (`ModuleLayer::infer_from_path`, `lint_parsed_source`).

Terminology: a "port" is a trait owned by the domain describing something the
domain needs; an "adapter" implements a port against real infrastructure. A
`OnceLock` is a standard-library cell that can be written once and read
globally thereafter.

Current Pattern B call sites, from `leta refs record_pagination_error`:

- `backend/src/inbound/http/users_pagination.rs:286,295` inside
  `map_cursor_error`, reached via `list_users` (`backend/src/inbound/http/
  users.rs:219`, which holds `web::Data<HttpState>`) →
  `parse_users_page_params` → `map_cursor_error`.
- `backend/src/outbound/persistence/user_persistence_error_mapping.rs:26,35`
  inside `map_user_pagination_error`, reached from
  `map_user_persistence_error`, whose callers include
  `diesel_login_service.rs` and the user repository adapter.

## Conformance basis

No Terms of Reference document exists for this work; the upstream artefacts
are:

- `docs/wildside-backend-architecture.md` — ARCH-PORTS-HOME: the 2025-11-09
  decision (lines ~1550–1555) that `backend::domain::ports` is the canonical
  home for port traits; the Observability and Telemetry section
  (lines ~2511–2654); the `RouteMetrics` sketch (lines ~2877–2880).
- `docs/execplans/backend-phase-1-idempotency-audit-metrics.md` —
  IDEM-SEPARATE-PORTS: the recorded decision to keep metrics ports separate
  per feature.
- `AGENTS.md` — style, documentation, and gate requirements.
- `docs/backend-roadmap.md` §6.2 — future observability extensions
  (exporters, structured logging); unaffected by this plan.

Trace links:

```plaintext
ARCH-PORTS-HOME -> EP-M1 -> backend/src/domain/ports/pagination_error_metrics.rs
ARCH-PORTS-HOME -> EP-M2 -> backend/src/outbound/metrics/prometheus_pagination_errors.rs
EP-CONS-001 (single pattern) -> EP-M3 -> V-GLOBAL-GONE, V-BEHAVIOUR-SAME
EP-CONS-002 (no dead ports)  -> EP-M4 -> V-DEAD-PORT-GONE
EP-CONS-001 -> EP-M5 -> V-LINT-GUARD
```

## Verification plan

Axioms (not verified here): the `prometheus` crate correctly aggregates
`IntCounterVec` increments and renders them via `Registry::gather`;
`actix-web-prom` serves the registry at `/metrics`; `tracing` delivers
structured events to installed subscribers. Repository-owned logic is
verified against the real `prometheus::Registry`, not a mock.

- Obligation: V-BEHAVIOUR-SAME — recording a pagination error through the
  new port increments `wildside_pagination_errors_total` with labels
  `{code, source}` exactly as before, once per call.
  Method: parameterized unit test over the `PaginationErrorSource` ×
  detail-code partitions (`users_http`/`user_persistence` ×
  `invalid_cursor`/`unsupported_direction`), asserting gathered counter
  values against a fresh `Registry`.
  Rationale: the input space is a small finite partition; exhaustive
  enumeration is practical and property generation adds nothing.
  Domain: all four source/code combinations, plus a double-increment case
  asserting the value 2.0.
  Artefact: tests in
  `backend/src/outbound/metrics/prometheus_pagination_errors.rs`.
  Evidence: red — the test file fails to compile before the adapter exists;
  green — `make test` passes with the new tests listed.
  Non-vacuity: the existing regression test (currently
  `pagination_errors.rs:94–116`) is ported and must still assert a gathered
  value of exactly 1.0; a deliberate mutation (dropping the `.inc()` call)
  must make it fail with value 0.0.
- Obligation: V-LOG-ALWAYS — the structured log is emitted by the provided
  trait method regardless of implementation, including `NoOp`.
  Method: unit test using a captured `tracing` subscriber
  (`tracing::subscriber::with_default` with a collecting layer, as existing
  backend tests do) around `NoOpPaginationErrorMetrics::record`.
  Rationale: the invariant is about an observable side effect a type check
  cannot see.
  Domain: one call per source variant on the `NoOp` implementation.
  Artefact: tests in
  `backend/src/domain/ports/pagination_error_metrics.rs`.
  Evidence: red — test written against the not-yet-existing port fails to
  compile; green — passes; the assertion checks the `error_code` and
  `source` fields, not merely that some event fired.
  Non-vacuity: asserting on field values means an empty-log implementation
  or a wrong-label implementation both fail.
- Obligation: V-GLOBAL-GONE — no metrics state is reachable except through
  an injected implementation.
  Method: structural check: `rg -n "OnceLock" backend/src` shows no
  metrics-related hits; `rg -n "register_pagination_error_metrics"` returns
  nothing; compilation proves no dangling callers.
  Rationale: absence of a construct is a syntactic property; the compiler
  and a grep are proportionate.
  Non-vacuity: before EP-M3 the same greps return hits at the documented
  lines, so the check demonstrably distinguishes the two states.
- Obligation: V-FEATURE-OFF — the crate builds and tests pass without the
  `metrics` feature, and pagination errors still log.
  Method: `cargo check -p backend --no-default-features` (plus whatever
  feature set the Makefile gate uses) and the V-LOG-ALWAYS test, which is
  not feature-gated.
  Rationale: the previous design encoded feature-off behaviour in duplicate
  function bodies; the new design must encode it as `NoOp` selection at
  composition, verified by compiling both configurations.
  Non-vacuity: deleting the `NoOp` arm in `server/mod.rs` composition would
  fail this compile.
- Obligation: V-DEAD-PORT-GONE — `RouteMetrics`/`RouteMetricsError` no
  longer exist.
  Method: `rg -n "RouteMetrics" backend/src docs` returns only historical
  ExecPlan/architecture-changelog mentions; `make test` compiles.
  Non-vacuity: before EP-M4 the grep hits
  `backend/src/domain/ports/route_metrics.rs` and `ports/mod.rs:170`.
- Obligation: V-LINT-GUARD — `tools/architecture-lint` rejects `prometheus`
  imports outside `outbound::metrics`, `server`, and the binary entry
  point.
  Method: extend the lint's forbidden-roots table; add a red fixture case to
  its existing `rstest`/BDD suites (`tools/architecture-lint/src/tests.rs`,
  `tests/architecture_guardrails_bdd.rs`) showing an inbound file importing
  `prometheus` is rejected.
  Rationale: the lint already models per-layer forbidden crates; this is
  the cheapest durable guard against pattern regression.
  Non-vacuity: the new negative fixture must fail against the pre-change
  lint table (asserting the violation is reported for the intended layer
  and crate root), and the whole backend tree must pass the extended lint.

No other non-trivial invariants are introduced: the change moves an existing
counter behind a trait without altering concurrency, ordering, or persisted
state.

## Plan of work

Stage A (done): reconnaissance, this document.

Stage B/C per milestone, red tests before production code throughout.

EP-M1. Create `backend/src/domain/ports/pagination_error_metrics.rs`:
move `PaginationErrorSource` here (now `pub`), define the port, and a `NoOp`
implementation. Re-export from `backend/src/domain/ports/mod.rs`. The trait
(names indicative):

```rust
pub trait PaginationErrorMetrics: Send + Sync {
    /// Increment the pagination-error counter for `source`/`detail_code`.
    fn increment(&self, source: PaginationErrorSource, detail_code: &'static str);

    /// Emit the structured log, then increment.
    fn record(&self, source: PaginationErrorSource, detail_code: &'static str) {
        // tracing::info! with error_code, source, trace_id — moved verbatim
        // from observability::pagination_errors::record_pagination_error.
        self.increment(source, detail_code);
    }
}

pub struct NoOpPaginationErrorMetrics;
impl PaginationErrorMetrics for NoOpPaginationErrorMetrics {
    fn increment(&self, _: PaginationErrorSource, _: &'static str) {}
}
```

Red: the V-LOG-ALWAYS test first. Green: implement. The old module stays in
place and callers are untouched at this plateau.

EP-M2. Create `backend/src/outbound/metrics/prometheus_pagination_errors.rs`
mirroring `prometheus_idempotency.rs`: `PrometheusPaginationErrorMetrics::
new(registry: &Registry) -> Result<Self, prometheus::Error>` building the
`IntCounterVec` with the exact `Opts` and label order from the current
`register_pagination_error_metrics`; `increment` calls
`with_label_values([detail_code, source.as_str()]).inc()` (preserve this
argument order — labels are declared `["code", "source"]`). Register in
`backend/src/outbound/metrics/mod.rs` behind the same feature gating as its
siblings. Red: the V-BEHAVIOUR-SAME parameterized tests. Green: implement.

EP-M3. Wiring and deletion, one coherent change:

- Add the handle to inbound state: extend `HttpState` (found via
  `backend/src/inbound/http/users.rs:219`) with
  `pagination_error_metrics: Arc<dyn PaginationErrorMetrics>`; pass it from
  `list_users` through `parse_users_page_params` into `map_cursor_error`
  (signatures gain one `&dyn PaginationErrorMetrics` parameter; callers are
  all in `backend/src/inbound/http/`).
- Outbound: give the Diesel adapters that call
  `map_user_persistence_error` an `Arc<dyn PaginationErrorMetrics>` field,
  set at construction in `backend/src/server/mod.rs`; the mapping functions
  in `user_persistence_error_mapping.rs` gain a `&dyn
  PaginationErrorMetrics` parameter.
- Composition: in `backend/src/server/mod.rs`, select
  `PrometheusPaginationErrorMetrics::new(&prom.registry)` when metrics are
  enabled, else `NoOpPaginationErrorMetrics`, alongside the existing
  idempotency selection. Remove the
  `backend::register_pagination_error_metrics(&metrics.registry)?` call from
  `backend/src/main.rs` and the re-export from `backend/src/lib.rs`.
- Delete `backend/src/observability/pagination_errors.rs` (and the
  `observability` module if empty apart from it; check
  `backend/src/observability/` for siblings first — `leta files
  backend/src/observability`).
- Update tests that constructed the global (the `#[serial]` test moves to
  the EP-M2 adapter tests; BDD steps referencing the counter re-assert via
  an injected adapter and fresh registry).

EP-M4. Delete `backend/src/domain/ports/route_metrics.rs` and its `pub use`
in `ports/mod.rs:170`. Update `docs/wildside-backend-architecture.md`: drop
or annotate the `RouteMetrics` sketch so the document no longer describes a
trait the code does not have; note that a route-cache metrics port will be
reintroduced with the cache itself.

EP-M5. Documentation and guardrail:

- Add a "Metrics conventions" subsection to `docs/developers-guide.md`
  (near the existing route-queue metrics section at ~line 1266): one
  pattern, port in `domain::ports`, `NoOp` default, Prometheus adapter in
  `outbound::metrics`, injection at the composition root, no global metric
  state.
- Extend `tools/architecture-lint` per V-LINT-GUARD (red fixture first).
- Update `docs/wildside-backend-architecture.md` Observability section to
  describe pagination-error metrics in their new home.

Delegate mechanical documentation edits to `scribe`; run gates via
`scrutineer` after each milestone.

## Milestones and plateaus

- EP-M1 — port and `NoOp` exist and are tested; no caller changed.
  Acceptance: `make test` green; V-LOG-ALWAYS discharged.
  Conformance: port lives in `domain::ports` (ARCH-PORTS-HOME); no new
  public interface beyond the crate-internal port. Recovery: revert the two
  new files. Remaining: Pattern B still live.
- EP-M2 — Prometheus adapter exists and is tested against a fresh registry;
  still uncalled. Acceptance: V-BEHAVIOUR-SAME discharged. Recovery: revert.
  Remaining: wiring.
- EP-M3 — single pattern in force; global deleted. Acceptance:
  V-GLOBAL-GONE, V-FEATURE-OFF, full gates green, `/metrics` still exposes
  `wildside_pagination_errors_total` (manual check: run the server with
  metrics enabled, trigger an invalid cursor on the users list endpoint,
  `curl /metrics | grep pagination`). Conformance: no persisted/wire format
  change (metric name and labels preserved). Recovery: this milestone is a
  single commit; revert restores Pattern B intact.
  Compatibility decision: none — `register_pagination_error_metrics` is
  application-internal and pre-1.0; no shim.
- EP-M4 — dead port removed; architecture doc consistent with code.
  Acceptance: V-DEAD-PORT-GONE; `make markdownlint` and `make nixie` green.
  Recovery: revert.
- EP-M5 — convention documented; lint guard active. Acceptance:
  V-LINT-GUARD; docs gates green. Recovery: revert.

Each milestone is committed separately with gates run beforehand.

## Concrete steps

All commands from the repository root. Gate runs are delegated to
`scrutineer`; when run directly, use `tee`:

```bash
make test 2>&1 | tee "/tmp/test-wildside-feat-competing-metrics-patterns.out"
make lint 2>&1 | tee "/tmp/lint-wildside-feat-competing-metrics-patterns.out"
make check-fmt 2>&1 | tee "/tmp/fmt-wildside-feat-competing-metrics-patterns.out"
make markdownlint 2>&1 | tee "/tmp/mdlint-wildside-feat-competing-metrics-patterns.out"
make nixie 2>&1 | tee "/tmp/nixie-wildside-feat-competing-metrics-patterns.out"
```

Note: `make check-fmt`/`make lint` can exit 0 despite sub-check failures —
read the log, not the exit code. The local Whitaker Dylint suite (0.2.7) is
ahead of CI's pin (0.2.6), so `make lint` may be red on a clean tree;
compare against a pre-change baseline run before attributing failures to
this work.

Feature-off check (V-FEATURE-OFF):

```bash
cargo check -p backend --no-default-features 2>&1 | \
  tee "/tmp/check-nofeat-wildside-feat-competing-metrics-patterns.out"
```

Expected red-stage transcript shape (EP-M2, before the adapter exists):

```plaintext
error[E0432]: unresolved import `crate::outbound::metrics::PrometheusPaginationErrorMetrics`
```

## Validation and acceptance

- Red: each milestone's new test compiles against a missing type or asserts
  behaviour the code does not yet have; run the focused test
  (`cargo nextest run -p backend <filter>` or `make test`) and record the
  failure reason in `Progress`.
- Green: minimal implementation; focused test passes.
- Refactor: tidy, then full gates via `scrutineer`.
- Done means: all six verification obligations discharged with evidence
  recorded here; `make test`, `make lint` (against baseline),
  `make check-fmt`, `make markdownlint`, `make nixie` green; behavioural
  check of `/metrics` performed at EP-M3; docs updated; five commits (one
  per milestone) with imperative, wrapped messages.

## Idempotence and recovery

Every milestone is a self-contained commit; `git revert` of any single
milestone restores the previous plateau. No data migrations, no persisted
state. Gate log files under `/tmp` are scratch and may be deleted.

## Artefacts and notes

Evidence gathered during reconnaissance (2026-08-16):

- Pattern A: `backend/src/domain/ports/idempotency_metrics.rs:38`
  (`trait IdempotencyMetrics`, `NoOpIdempotencyMetrics` at :62),
  `route_queue_metrics.rs:29`, `enrichment_job_metrics.rs:63` (with
  `mockall::automock`); adapters
  `backend/src/outbound/metrics/prometheus_idempotency.rs:36`,
  `prometheus_route_queue.rs:37`, `prometheus_enrichment_jobs.rs:18`;
  selection in `backend/src/server/mod.rs:98–133`.
- Pattern B: `backend/src/observability/pagination_errors.rs:21`
  (`OnceLock` static), `:42` (`record_pagination_error`), `:67`
  (`register_pagination_error_metrics`); call sites
  `backend/src/inbound/http/users_pagination.rs:286,295` and
  `backend/src/outbound/persistence/user_persistence_error_mapping.rs:26,35`;
  crate-root re-export `backend/src/lib.rs:47`; start-up call
  `backend/src/main.rs:31`.
- Dead port: `backend/src/domain/ports/route_metrics.rs:15`; only other
  reference is the re-export at `ports/mod.rs:170` (`leta refs
  RouteMetrics`).

## Interfaces and dependencies

No new dependencies. End-state interfaces:

- `backend/src/domain/ports/pagination_error_metrics.rs`:
  `pub enum PaginationErrorSource { UsersHttp, UserPersistence }` (with
  `as_str` yielding `"users_http"`/`"user_persistence"`),
  `pub trait PaginationErrorMetrics: Send + Sync` as sketched in the plan of
  work, `pub struct NoOpPaginationErrorMetrics`.
- `backend/src/outbound/metrics/prometheus_pagination_errors.rs`
  (feature `metrics`):
  `pub struct PrometheusPaginationErrorMetrics { counter: IntCounterVec }`,
  `pub fn new(registry: &prometheus::Registry) -> Result<Self, prometheus::Error>`.
- `backend/src/server/mod.rs`: composition selects the implementation; the
  handle travels as `Arc<dyn PaginationErrorMetrics>`.
- Deleted: `backend/src/observability/pagination_errors.rs`,
  `backend/src/domain/ports/route_metrics.rs`, the `lib.rs` re-export, and
  the `main.rs` registration call.
