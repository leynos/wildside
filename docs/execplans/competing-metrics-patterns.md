# Consolidate the backend's two competing metrics patterns

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
`Outcomes & Retrospective`, `Conformance Basis`, and `Verification Plan` must
be kept up to date as work proceeds.

Status: IMPLEMENTATION COMPLETE; the Rustls audit repair passes local gates and
CodeScene. Commit and CodeRabbit CLI re-review, hosted CI, and the hosted
ExecPlan assessment remain pending on draft PR #460 stacked on PR #542.

## Purpose / big picture

The backend records application metrics in two structurally incompatible ways.

Pattern A is hexagonal: the domain defines a metrics port (a trait in
`backend/src/domain/ports/`), a `NoOp*` implementation serves tests and
metrics-disabled builds, and a `Prometheus*` adapter in
`backend/src/outbound/metrics/` implements the trait against an explicitly
injected `prometheus::Registry`. `IdempotencyMetrics`, `RouteQueueMetrics`, and
`EnrichmentJobMetrics` all follow this shape, and the implementation is
selected at server-composition time in `backend/src/server/mod.rs`.

Pattern B is a module-level global. Its
`backend/src/observability/pagination_errors.rs` file holds a
`static PAGINATION_ERRORS_TOTAL: OnceLock<IntCounterVec>`, populated at process
start by `register_pagination_error_metrics(&Registry)` (re-exported at the
crate root and called from `backend/src/main.rs`), and incremented from
anywhere via the bare free function `record_pagination_error`. Feature gating
is done by duplicating function bodies under `#[cfg(feature = "metrics")]` /
`#[cfg(not(...))]` instead of a `NoOp` implementation. The function is called
directly from an inbound HTTP adapter
(`backend/src/inbound/http/users_pagination.rs`) and an outbound persistence
adapter (`backend/src/outbound/persistence/user_persistence_error_mapping.rs`).

The completed change leaves one pattern: every metric is recorded through an
injected trait with a `NoOp` and a Prometheus implementation, the `OnceLock`
global and the crate-root registration function are gone, and the convention is
written down in `docs/developers-guide.md`. Observable success:
`rg "OnceLock" backend/src` returns no metrics-related hits, the
`wildside_pagination_errors_total` counter still increments with the same name
and labels for the same events, and all commit gates pass. The plan also
resolves the dead `RouteMetrics` port (defined and re-exported in
`backend/src/domain/ports/route_metrics.rs` but never implemented or called) by
removing it.

## Constraints

- The exported metric name `wildside_pagination_errors_total` and its label
  set `["code", "source"]` must not change; dashboards and alerting treat the
  exposition format as a wire format.
- The structured `tracing::info!` log emitted when a pagination error is
  mapped must continue to be emitted in all builds, including builds without the
  `metrics` cargo feature.
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

- Scope: stop and escalate if the change exceeds 30 files or 600 net lines.
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

- Risk: the persistence error-mapping function is called from three adapters,
  including `diesel_user_profile_query.rs`, which the initial reconnaissance
  missed. Threading an injected handle therefore reaches one more adapter than
  the plan first mapped. Severity: medium. Likelihood: confirmed. Mitigation:
  inject the same `Arc<dyn PaginationErrorMetrics>` into all three adapters and
  record each mapping call in the caller inventory. The user later approved 30
  paths for roadmap closure; the 600-net-line limit remains hard.
- Risk: `serial_test`-guarded tests depend on the `OnceLock` global's
  process-wide lifetime; removing the global changes test isolation
  assumptions. Severity: low. Likelihood: medium. Mitigation: the replacement
  adapter takes `&Registry` at construction, so each test builds a fresh
  `Registry` and the `#[serial]` guard can be dropped.
- Risk: removing `RouteMetrics` conflicts with the architecture document,
  which sketches it as intended design (`docs/wildside-backend-architecture.md`
  around lines 2877–2880). Severity: low. Likelihood: high. Mitigation: EP-M4
  updates the architecture document in the same commit and records the removal
  in the decision log; the trait is trivially re-introducible when a route
  cache lands.
- Risk: hosted CI and the ExecPlan assessment await publication of the audit
  repair; its local deterministic gates and CodeScene checks pass. Severity:
  low. Likelihood: pending external checks. Mitigation: commit the validated
  lock-only repair, re-review that exact candidate, then hand it to
  `pr-babysitting` for hosted CI and assessment.

## Progress

- [x] (2026-08-16) Reconnaissance complete: both patterns characterized with
  file:line evidence; `RouteMetrics` confirmed dead (definition and `pub use`
  only, no implementations or call sites).
- [x] (2026-09-28–2026-10-15) Preflight found a third adapter; user raised
  scope from 20 to 25, 29 for M5, then 30 paths; 600 lines stayed fixed.
- [x] (2026-10-15) Fixed CodeScene's 74-line test (scenario 49, helpers
  26/18); rebased 18 owner-authored commits onto PR #542 (`6ec7f72`);
  range-diff matched all. Gates, metrics-off, CodeScene 10.0/clean delta and
  compile-fail tests passed; Nextest 1,810 passed, four skipped.
- (2026-10-15) The configured spelling gate regenerated the colour-name
  expression in `typos.toml`. Following `AGENTS.md`, this unrelated generated
  refresh is isolated in draft base PR #542 at `6ec7f72`; its formatting,
  spelling/Markdown, and Mermaid gates passed. Stack this task on #542 while
  retaining the approved 30-path/600-net-line combined cap.
- (2026-10-13) EP-M1 red-stage tests were added before the port implementation.
  Cargo spent 5h28m preparing shared target artefacts and compiling uncached
  because sccache was unreachable. The filtered test completed successfully
  against the now-implemented port: both HTTP and persistence source-label
  assertions passed. Since the port was added while Cargo prepared
  dependencies, the expected red result was not observed. Evidence:
  `/tmp/test-red-m1-fc6a3bee-d4dd-4e85-bad9-aab0e5d668a4-feat-competing-metrics-patterns-1791851016.out`.
- [x] EP-M1: pagination metrics port and `NoOp` implementation are added; the
  focused test and all deterministic milestone gates pass.
- (2026-10-13) Initial M1 gates: formatting passed; lint found a missing test
  module doc comment, later fixed. Its Python tier hit HTTP 403 through the
  Lody GitHub helper. A clean PATH still routed through `GIT_EXEC_PATH`;
  unsetting it let standard Git's public `ls-remote` succeed. Evidence:
  `/tmp/lint-fc6a3bee-d4dd-4e85-bad9-aab0e5d668a4-feat-competing-metrics-patterns-5.out`.
- (2026-10-13) With standard Git routing restored, `make lint` passed rustdoc,
  Clippy, Whitaker, architecture, frontend and specification checks, then
  Pylint found C1803 in the unchanged
  `tests/workflow_contracts/makefile_tooling_test.py`. The equivalent
  truth-value assertion is simplified so the full deterministic lint gate can
  pass; this adds one file to the branch's counted scope.
- (2026-10-13) The clean-routed `make lint` retry passed, including the pinned
  Pylint dependency, and `make test` passed 1,482 Nextest tests plus the Bun
  and Python suites. `make markdownlint` then found the American spelling
  `artifacts` above; this is corrected to `artefacts`. Markdown and Mermaid
  gates remain pending.
- (2026-10-13) Final EP-M1 gates passed: `make check-fmt`, `make lint`,
  `make test`, `make markdownlint`, and `make nixie`. The test run covered
  1,482 Nextest tests and the remaining Bun and Python suites. The
  documentation gates passed after correcting the spelling finding. The
  deterministic gate run recorded no findings.
- (2026-10-13) CodeRabbit reviewed M1 at `cdc25ac` and reported one low-severity
  documentation finding: a stray space in the `users.rs` path below. The path
  is corrected. `make check-fmt`, `make markdownlint`, and `make nixie` passed
  after the correction. A final CodeRabbit review at `c57a3d2` reported zero
  findings, clearing the concern before EP-M2.
- (2026-10-13) EP-M2 parameterized tests were added for all source/detail-code
  combinations, the metric name/help/label order, and a double increment. The
  focused metrics-enabled Cargo run reached the expected unresolved adapter
  re-export before implementation. It compiled uncached while waiting for the
  shared package lock because sccache was unreachable. Evidence:
  `/tmp/test-red-m2-fc6a3bee-d4dd-4e85-bad9-aab0e5d668a4-feat-competing-metrics-patterns.out`.
- (2026-10-13) After implementing the adapter, the focused metrics-enabled
  library test passed all five cases (four source/code combinations and a
  double increment). The gathered samples preserve the metric name, help, label
  names/order, label values, and counts. Evidence:
  `/tmp/test-m2-focused-fc6a3bee-d4dd-4e85-bad9-aab0e5d668a4-feat-competing-metrics-patterns-2.out`.
- (2026-10-13) EP-M2 full deterministic gates passed on the implementation
  candidate at `f944360` (the source changes were present in the worktree).
  `make check-fmt`, `make lint`, `make test`, `make markdownlint`, and
  `make nixie` all passed. The lint log includes one existing non-failing
  workflow truthy-value warning at
  `.github/workflows/dependabot-automerge.yml:7`. The test run passed 1,487
  Nextest cases, the compile-fail harness, Bun checks, workflow contracts, and
  107 Python tests. Evidence:
  `/tmp/check-fmt-fc6a3bee-d4dd-4e85-bad9-aab0e5d668a4-feat-competing-metrics-patterns-7.out`,
  `/tmp/lint-fc6a3bee-d4dd-4e85-bad9-aab0e5d668a4-feat-competing-metrics-patterns-5.out`,
  `/tmp/test-fc6a3bee-d4dd-4e85-bad9-aab0e5d668a4-feat-competing-metrics-patterns-2.out`,
  `/tmp/markdownlint-fc6a3bee-d4dd-4e85-bad9-aab0e5d668a4-feat-competing-metrics-patterns-9.out`,
  and
  `/tmp/nixie-fc6a3bee-d4dd-4e85-bad9-aab0e5d668a4-feat-competing-metrics-patterns-8.out`.
- (2026-10-13) CodeRabbit reviewed the committed M2 candidate
  `706f82754e9056fcd218c4b1ef561e8317f9220f`; the review completed with zero
  findings across all six changed files. Evidence:
  `/tmp/coderabbit-fc6a3bee-d4dd-4e85-bad9-aab0e5d668a4-feat-competing-metrics-patterns-3.out`.
- [x] EP-M2: adapter, deterministic gates, and CodeRabbit review all pass.
- [x] EP-M2: Prometheus adapter in `backend/src/outbound/metrics/`.
- (2026-10-13) EP-M3 wiring is in progress: `HttpState` and the three Diesel
  user adapters now receive one injected `PaginationErrorMetrics` handle from
  server composition; HTTP parsing and persistence mapping tests assert exact
  source/detail-code values with local recorders. The existing cursor response
  tests retain their serial guards and continue to verify the HTTP error
  contract; they no longer depend on a process-global recorder. The `OnceLock`
  module, root export and startup registration call are removed. Fourteen
  metrics-enabled parser, mapping, adapter and server-exposition tests passed,
  followed by two cursor-response tests. The exposition test starts the server,
  submits an invalid cursor, and confirms `/metrics` exposes the preserved
  counter name, labels and increment. Evidence:
  `/tmp/ep-m3-focused-final-fc6a3bee-d4dd-4e85-bad9-aab0e5d668a4.out` and
  `/tmp/ep-m3-cursor-http-fc6a3bee-d4dd-4e85-bad9-aab0e5d668a4.out`. A first
  focused build overlapped source edits and failed against an intermediate
  tree; it is not treated as validation. An initial exposition-test attempt
  timed out while stopping the server with the client pool alive; dropping the
  client before graceful shutdown resolved it.
- (2026-10-13) First M3 gate wave passed formatting, typecheck, tests (1,489
  Nextest, compile-fail 1/1, Python 107), Mermaid, and feature-off. Lint found
  HTTP imports in the outbound test and a 410-line state builder; spelling also
  saw an unstaged deletion. Fixes and rerun evidence follow below.
- (2026-10-13) Gate-driven corrections: the `/metrics` end-to-end test now
  lives in an inline test module in `backend/src/main.rs`, keeping inbound and
  HTTP dependencies out of `outbound::metrics` while using an already changed
  path. The test-support wrapper was initially moved to `server/mod.rs`, leaving
  `server/state_builders.rs` at 370 lines; its no-op import was removed. Only
  the two intentionally deleted observability paths were staged so the
  index-based spelling gate omitted them. The original candidate’s serial
  response-test attributes remain unchanged.
- (2026-10-13) Corrected M3 passed all seven gates: formatting, lint,
  typecheck, tests (1,489 Nextest; compile-fail 1/1; Python 107), Markdown,
  Mermaid, and feature-off. Captured gate logs are recorded in the ledger.
  CodeRabbit reviewed committed candidate `6ff878e`; findings and corrections
  are recorded below.
- (2026-10-13) CodeRabbit at `6ff878e` flagged a `dead_code` suppression and
  unbounded HTTP requests. Since the binary compiles its own server module, the
  helper moved to library `test_support.rs`; the exposition test stayed in
  `main.rs` to respect architecture and file limits. Its requests use a
  15-second bound, and server stop/join completes before errors propagate. An
  initial helper placement failed lint; the corrected candidate passed all
  seven gates (1,489 passed, four skipped) and CodeRabbit found zero issues at
  `70b740e`. Captured logs are recorded in the ledger.
- (2026-10-13) CodeRabbit re-reviewed the corrected M3 commit
  `70b740e23d8cca77fc1386426da5f5b7b0608877` and reported zero findings. Output:
  `/tmp/coderabbit-fc6a3bee-d4dd-4e85-bad9-aab0e5d668a4-feat-competing-metrics-patterns-2.out`.
- (2026-10-13) EP-M4 implementation removed the unused port definition and
  export, and revised the architecture decision, testing guidance, and route
  service example. A source-wide search now finds no code references; remaining
  documentation mentions are historical records in the implementation plan,
  completed roadmap, prior idempotency plan, and architecture history.
- (2026-10-13) The first M4 gate wave passed formatting, lint, typecheck, and
  tests, then stopped at spelling because the candidate deletion was not staged:
  `git ls-files` still supplied the removed path to typos. No spelling
  configuration change is needed; stage the candidate deletion before rerunning
  the index-based gate.
- (2026-10-14) M5 red tests failed as expected; lint exposed the outbound
  queue's Prometheus test, and Whitaker rejected the original three-branch
  condition. The final guard permits outbound metrics and outbound test paths.
  Focused tests passed (14 unit, two BDD, one doctest); full gates passed:
  1,495 Nextest, four skipped, 107 Python, compile-fail/frontend/workflow,
  format, lint, typecheck, Markdown, Mermaid and feature-off. Logs are the
  captured `*-3.out` gates and backend `*-2.out` check.
- (2026-10-13) M4 at `a870894` passed all seven gates (1,489 passed, four
  skipped; Markdown 125 files, zero errors); CodeRabbit found zero issues in 23
  files. Review log:
  `/tmp/coderabbit-fc6a3bee-d4dd-4e85-bad9-aab0e5d668a4-feat-competing-metrics-patterns-3.out`.
- [x] EP-M3: wire injection through inbound and outbound call sites; delete
  the `OnceLock` global, the duplicate `cfg` function bodies, the `lib.rs`
  re-export, and the `main.rs` registration call.
- [x] EP-M3: deterministic gates and CodeRabbit review clear.
- [x] EP-M4: remove the dead `RouteMetrics` port; update
  `docs/wildside-backend-architecture.md`.
- [x] EP-M4: deterministic gates and CodeRabbit review clear.
- [x] EP-M5: policy documented, lint guard and gates pass; CodeRabbit review
      clear.
- (2026-10-15) CodeRabbit found missing `actix_web_prom` guards in domain and
  inbound; allowing inbound conflicts with the developers' guide. Both red
  fixtures reproduced the gap; focused tests and final gates passed (Nextest
  1,812/4 skipped; pytest 606/1 skipped; metrics-off). CodeScene scored 10.0;
  Delta was clean. CodeRabbit CLI 0.9.0 reviewed 29 files at `0e98666` vs
  `6ec7f72`: zero findings; full comparison context is in the ledger.
- (2026-10-15–16) Published M1–M5 as draft PR #460 stacked on #542 at
  `6ec7f72916a0417fbf6cc8fa799ad6b1e671adf0`; candidate `0e98666` passed seven
  gates and CodeRabbit. Hosted run `38812258008` later failed Audit on
  `rustls 0.23.45`, while coverage passed. Four RustSec advisories identify
  `0.23.46` as patched; only `Cargo.lock` changes, the thirtieth path, with no
  new dependency or net-line growth. On dirty candidate HEAD
  `0d8035bcd1d5d2bcff5d4ee2c6e6e929f5d2cb4b`, the lock repair passed all gates:
  `make check-fmt`, `make lint`, `make typecheck`, `make test` (1,812 passed,
  four skipped; both compile-fail tests passed; Python and Bun suites passed),
  `make markdownlint`, `make nixie`, `make audit` (Bun reported no
  vulnerabilities; Rust reported 15 allowlisted warnings), and
  `cargo check -p backend --no-default-features`. CodeScene scored 10.0 and
  found no delta issues against the PR base. The subsequent plan-only update
  passed `make check-fmt`, `make markdownlint`, and `make nixie`; exact
  candidate fingerprints and logs are in the delivery ledger. Commit and
  CodeRabbit review of this exact repair, hosted CI rerun, and the ExecPlan
  assessment remain pending; proof assessment is N/A.
- (2026-10-16) CodeRabbit reviewed `ce17ca8` against the exact stacked base and
  found `tools/architecture-lint/src/lib.rs` one line over the 400-line file
  limit. Inlining the one-use predicate then failed Whitaker's
  `conditional-max-n-branches` lint. The corrected candidate restores the named
  predicate and shortens the module overview; the allowlist is unchanged and
  the file is exactly 400 lines. Candidate diff fingerprint
  `01c8b0500389255f1bbcf1993638a7b69fc473a4ef01e7967755f789440e0a97` passed
  `make check-fmt`, `make lint`, `make typecheck`, `make test` (1,812 Nextest
  cases, four skipped; compile-fail, Python, frontend, workflow, and
  action-lint suites passed), `make markdownlint` (126 files, zero errors),
  `make nixie`, `make audit` (15 existing allowlisted advisories), and the
  backend no-default-features check. CodeScene scored 10.0 and found no delta
  issues. Exact logs are in the delivery ledger. Only committed-head CodeRabbit
  review remains before publication.
- (2026-10-16) CodeRabbit reviewed `78c4095` and questioned whether the path
  prefix also admits `outbound/metrics_backup.rs`. Locked Camino 1.2.4
  documents that `Utf8Path::starts_with` compares whole path components and
  delegates to `std::path::Path::starts_with`; the sibling path does not match.
  Added the requested near-miss regression case expecting a Prometheus import
  there to remain forbidden. No production rule change is needed. The focused
  test passed 17/17 cases (four skipped); the full gate wave passed on
  fingerprint
  `14c108716dd396456f2ffbe07f2582e3427e0892f48d9c6637c0fbbc425cd619`: 1,813
  Nextest cases, compile-fail, Python, frontend, workflow, and action-lint
  tests, formatting, lint, typecheck, Markdown, Mermaid, audit, and metrics-off
  all passed. CodeScene scored 10.0 with no delta issues. Logs are in the
  delivery ledger. Committed-head CodeRabbit re-review remains pending.
- (2026-10-16) CodeRabbit reviewed `1adddde` and reported two trivial
  findings. Camino path semantics and the new `metrics_backup.rs` regression
  had already established the directory-prefix behaviour. Added a second
  negative case for `outbound/persistence/tests_helper.rs`, which is a file
  name rather than a `tests/` directory, and documented the deliberately
  unsupported `dir: "Sideways"` cursor fixture. The first `make markdownlint`
  reported one en-GB spelling error in this entry; the correction is included
  here. At dirty-patch fingerprint
  `b6c446e212f65a5e9bc588e5793cf32349b172b21e1ae1c1ac4842eeab118431`, the
  focused boundary suite passed 18 cases (four skipped), and all gates passed:
  `make check-fmt`, `make lint`, `make typecheck`, `make test`,
  `make markdownlint`, `make nixie`, `make audit`, and
  `cargo check -p backend --no-default-features`. Nextest passed 1,814 tests
  with four skipped; both compile-fail harnesses passed; Python workflow
  contracts passed 115 with 13 skipped; action-lint passed 14. Bun audit found
  no vulnerabilities across 688 packages (seven ignored), and Rust audit
  reported 16 allowed warnings. CodeScene scored 10.0 and found no delta issues
  against stacked base `6ec7f72`. The worktree still has only the three
  intended paths. Against the stacked base, the branch has 30 unique paths and
  432 net implementation lines excluding this living ExecPlan, within the
  approved 30-path/600-line cap. Committed-head CodeRabbit re-review remains
  pending.
- (2026-10-16) The final ExecPlan closeout passed `make check-fmt` (125 files
  unchanged, 113 already formatted), `make markdownlint` (126 files, zero
  errors), and `make nixie` (all diagrams validated). No generated tracked
  files changed. Logs use the `-docs-closeout-1.out` suffix in `/tmp`.

## Surprises & discoveries

- Observation: repository docs had no named metrics-pattern conflict; only
  per-feature ports were documented. The task and full-tree Markdown sweep
  established this plan as the record of the divergence and its resolution.
- Observation: roadmap §4.2.3 covers page-size, cursor-direction, and traversal
  analytics, not pagination-error metrics architecture. Added completed task
  1.3.4 under architecture guardrails for this implementation.
- Observation (2026-10-15–16): base PR #542 isolates spelling refresh; hosted
  audit found four `rustls 0.23.45` advisories, patched in `0.23.46`.
- Observation (2026-10-16): the lock-only upgrade passes `make audit`; Bun found
  no vulnerabilities across 688 packages (seven ignored), and the Rust audit
  retained 15 allowlisted warnings. No dependency manifest or audit exception
  changed.
- Observation: both patterns already share one `prometheus::Registry`, owned
  by `actix_web_prom::PrometheusMetrics` built in `backend/src/main.rs` with
  endpoint `/metrics`. Evidence: `backend/src/main.rs` lines 28–31. Impact:
  consolidation changes recording style only; exposition is untouched.
- Observation: `map_user_persistence_error` is also called by
  `DieselUserProfileQuery`, in addition to `DieselLoginService` and
  `DieselUsersQuery`. Evidence: source-reference search and CodeGraph edit
  context for
  `backend/src/outbound/persistence/user_persistence_error_mapping.rs`. Impact:
  EP-M3 must inject the pagination metrics port into all three adapters to
  preserve recording for every mapping call.
- Observation: `build_http_state` in `backend/src/server/state_builders.rs`
  constructs the HTTP state and all three Diesel user adapters before
  `create_server` builds the Actix server in `backend/src/server/mod.rs`.
  Impact: EP-M3 must thread the selected handle through this builder as well as
  the HTTP state; selection remains at server composition time.
- Observation (2026-10-13): the HTTP cursor-error tests and persistence error
  mapping tests were serialized around the module-level recorder, but no BDD
  step asserts on the Prometheus counter. The users-list BDD scenario checks
  the invalid-cursor response only. Impact: M3 replaces recorder-dependent
  assertions with injected-recorder assertions in unit tests. The cursor
  response tests retain their serial attributes and response assertions.

## Decision log

- Decision: adopt Pattern A (port + `NoOp` + Prometheus adapter, injected at
  composition time) as the single sanctioned metrics pattern, and migrate
  pagination-error recording to it. Rationale: Pattern A already covers three
  of four metric families, keeps the domain free of `prometheus` types, gives
  per-test registry isolation, and avoids the `OnceLock` global's hidden
  temporal coupling (silently recording nothing if registration is forgotten).
  The alternative — blessing the global free-function style — would spread
  implicit global state and duplicate-`cfg` bodies to every future metric.
  Date/Author: 2026-08-16, drafting agent.
- Decision: give the pagination port a provided method that always emits the
  structured `tracing` log and then calls a required `increment` hook, instead
  of splitting logging from metrics at every call site. Rationale: preserves
  the existing log-always, count-when-enabled semantics in one place;
  `NoOpPaginationErrorMetrics` gets logging for free. Date/Author: 2026-08-16,
  drafting agent.
- Decision: the pagination port is synchronous and infallible
  (`fn`, returns `()`), unlike `IdempotencyMetrics` (async, fallible).
  Rationale: its call sites are synchronous error-mapping functions on the
  request path; counter increments cannot meaningfully fail, and a metrics
  failure must never alter request handling. Matches the current behaviour of
  `increment_pagination_error_counter`. Date/Author: 2026-08-16, drafting agent.
- Decision (2026-10-13): keep the end-to-end `/metrics` assertion with the
  binary's inline test module in `backend/src/main.rs` rather than the outbound
  Prometheus adapter tests. Rationale: the architecture guard correctly forbids
  an outbound metrics module from depending on inbound HTTP and Actix client
  APIs. The test-support wrapper lives in the library's `test_support.rs` and
  calls the shared state-builder seam. This avoids a dead-code diagnostic
  because `main.rs` compiles its own `server` module, without restoring a lint
  suppression. Keep the test in this module and extract focused helpers to
  satisfy CodeScene without weakening the bounded request or shutdown checks.
  Date/Author: 2026-10-13 and 2026-10-15, implementation agent.
- Decision (2026-10-15): use Git's built-in `zdiff3` merge during the rebase
  because Weave was selected only by global attributes, not repository policy.
  Preserve target-side guide changes and replay only the metrics convention.
  Date/Author: 2026-10-15, implementation agent.
- Decision: remove `RouteMetrics` and `RouteMetricsError` rather than wire
  them. Rationale: pre-1.0, application-internal, zero implementations and zero
  call sites; a dead port invites divergent guesses about its semantics.
  `AGENTS.md` requires sweeping for existing abstractions before adding new
  ones — a phantom abstraction poisons that sweep. The architecture document is
  updated in the same milestone so design and code agree. Date/Author:
  2026-08-16, drafting agent.
- Decisions (2026-10-13–15): the user raised the path cap from 20 to 25, 29
  for EP-M5, then 30 for roadmap closeout. The 600-line limit and verification
  obligations remain unchanged. No architecture or ADR change was required.
  Author: User.
- Decision (2026-10-15): reject `actix_web_prom` in domain and inbound modules,
  following the developers' guide and the existing `prometheus` boundary; the
  conflicting reviewer suggestion to allow it inbound was rejected. Keep the
  crate available at the composition root.
- Decisions (2026-10-15–16): isolate generated spelling refresh on base PR #542.
  After the hosted audit, update only locked `rustls` to patched `0.23.46`, the
  first release allowed by existing constraints; add no dependency and do not
  expand the audit allowlist. `make audit` confirms this resolves the new
  findings while the existing 15 allowed warnings remain. User authorized the
  scope; implementation agent selected the lock-only repair.
- Decision (2026-10-16): resolve the architecture-lint line-limit finding
  within the existing 30-path cap by clarifying the module overview while
  retaining the named metrics/test path predicate. Rationale: Whitaker rejects
  the inlined compound condition; the named predicate preserves the rule and
  keeps the file within 400 lines without adding a path. Date/Author:
  2026-10-16, implementation agent.
- Decision (2026-10-16): retain Camino's `Utf8Path::starts_with` directory
  check and add a `metrics_backup.rs` near-miss test. Rationale: the locked
  Camino implementation matches whole path components, so this path is not
  admitted by the metrics directory exception. The regression case records that
  contract without changing the production rule. Date/Author: 2026-10-16,
  implementation agent.
- Decision (2026-10-16): preserve the `/tests/` directory exception and add a
  negative case for `outbound/persistence/tests_helper.rs`. Rationale: the
  boundary allows Prometheus in outbound test directories, not files whose
  names merely contain `tests`; the fixture protects that distinction.
  Date/Author: 2026-10-16, implementation agent.

## Outcomes & retrospective

EP-M1 established the infallible port, preserved logging and labels, and passed
gates. The unrelated Pylint assertion fix remains within scope; CodeRabbit
cleared M1.

EP-M1–M5 and roadmap task 1.3.4 are complete. The counter contract is
preserved, globals and the dead port are gone, and architecture lint enforces
the documented boundary. The lock-only Rustls repair passes all local gates,
including `make audit` and the metrics-off check; CodeScene is clear. No proof
changed, so proof assessment is N/A. After committing and reviewing this
repair, hosted CI and the ExecPlan assessment remain with `pr-babysitting`; the
pull request remains a draft.

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
  implementations based on configuration and the `metrics` cargo feature (see
  `build_route_submission_service`, lines ~98–133, matching on
  `(config.db_pool, config.metrics())`).
- `backend/src/observability/pagination_errors.rs` — the Pattern B module
  this plan dissolves (117 lines: enum `PaginationErrorSource`, free functions
  `record_pagination_error`, `increment_pagination_error_counter` ×2,
  `register_pagination_error_metrics`, one `#[serial]` test).
- `tools/architecture-lint/` — a `syn`-based lint binary that walks
  `backend/src` and rejects forbidden imports per layer
  (`ModuleLayer::infer_from_path`, `lint_parsed_source`).

Terminology: a "port" is a trait owned by the domain describing something the
domain needs; an "adapter" implements a port against real infrastructure. A
`OnceLock` is a standard-library cell that can be written once and read
globally thereafter.

Current Pattern B call sites, from CodeGraph symbol search/edit context and an
exact source-reference sweep (the `leta` tool is not installed):

- `backend/src/inbound/http/users_pagination.rs:286,295` inside
  `map_cursor_error`, reached via `list_users`
  (`backend/src/inbound/http/users.rs:219`, which holds `web::Data<HttpState>`)
  → `parse_users_page_params` → `map_cursor_error`.
- `backend/src/outbound/persistence/user_persistence_error_mapping.rs:26,35`
  inside `map_user_pagination_error`, reached from
  `map_user_persistence_error`, whose callers are `diesel_login_service.rs`,
  `diesel_users_query.rs`, and `diesel_user_profile_query.rs`.

## Conformance basis

No Terms of Reference document exists for this work; the upstream artefacts are:

- `docs/wildside-backend-architecture.md` — ARCH-PORTS-HOME: the 2025-11-09
  decision (lines ~1550–1555) that `backend::domain::ports` is the canonical
  home for port traits; the Observability and Telemetry section (lines
  ~2511–2654); the `RouteMetrics` sketch (lines ~2877–2880).
- `docs/execplans/backend-phase-1-idempotency-audit-metrics.md` —
  IDEM-SEPARATE-PORTS: the recorded decision to keep metrics ports separate per
  feature.
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
structured events to installed subscribers. Repository-owned logic is verified
against the real `prometheus::Registry`, not a mock.

- Obligation: V-BEHAVIOUR-SAME — recording a pagination error through the
  new port increments `wildside_pagination_errors_total` with labels
  `{code, source}` exactly as before, once per call. Method: parameterized unit
  test over the `PaginationErrorSource` × detail-code partitions (`users_http`/
  `user_persistence` × `invalid_cursor`/`unsupported_direction`), asserting
  gathered counter values against a fresh `Registry`. Rationale: the input
  space is a small finite partition; exhaustive enumeration is practical and
  property generation adds nothing. Domain: all four source/code combinations,
  plus a double-increment case asserting the value 2.0. Artefact: tests in
  `backend/src/outbound/metrics/prometheus_pagination_errors.rs`. Evidence: red
  — the test file fails to compile before the adapter exists; green —
  `make test` passes with the new tests listed. Non-vacuity: the existing
  regression test (currently `pagination_errors.rs:94–116`) is ported and must
  still assert a gathered value of exactly 1.0; a deliberate mutation (dropping
  the `.inc()` call) must make it fail with value 0.0.
- Obligation: V-LOG-ALWAYS — the structured log is emitted by the provided
  trait method regardless of implementation, including `NoOp`. Method: unit
  test using a captured `tracing` subscriber
  (`tracing::subscriber::with_default` with a collecting layer, as existing
  backend tests do) around `NoOpPaginationErrorMetrics::record`. Rationale: the
  invariant is about an observable side effect a type check cannot see. Domain:
  one call per source variant on the `NoOp` implementation. Artefact: tests in
  `backend/src/domain/ports/pagination_error_metrics.rs`. Evidence: red — test
  written against the not-yet-existing port fails to compile; green — passes;
  the assertion checks the `error_code` and `source` fields, not merely that
  some event fired. Non-vacuity: asserting on field values means an empty-log
  implementation or a wrong-label implementation both fail.
- Obligation: V-GLOBAL-GONE — no metrics state is reachable except through
  an injected implementation. Method: structural check:
  `rg -n "OnceLock" backend/src` shows no metrics-related hits;
  `rg -n "register_pagination_error_metrics"` returns nothing; compilation
  proves no dangling callers. Rationale: absence of a construct is a syntactic
  property; the compiler and a grep are proportionate. Non-vacuity: before
  EP-M3 the same greps return hits at the documented lines, so the check
  demonstrably distinguishes the two states.
- Obligation: V-FEATURE-OFF — the crate builds and tests pass without the
  `metrics` feature, and pagination errors still log. Method:
  `cargo check -p backend --no-default-features` (plus whatever feature set the
  Makefile gate uses) and the V-LOG-ALWAYS test, which is not feature-gated.
  Rationale: the previous design encoded feature-off behaviour in duplicate
  function bodies; the new design must encode it as `NoOp` selection at
  composition, verified by compiling both configurations. Non-vacuity: deleting
  the `NoOp` arm in `server/mod.rs` composition would fail this compile.
- Obligation: V-DEAD-PORT-GONE — `RouteMetrics`/`RouteMetricsError` no longer
  exist in source or current design examples. Method:
  `rg -n "RouteMetrics" backend/src docs` returns no source hits and only
  historical plan, roadmap, and architecture-decision records; `make test`
  compiles. Non-vacuity: before EP-M4 the grep hits
  `backend/src/domain/ports/route_metrics.rs` and `ports/mod.rs:170`.
- Obligation: V-LINT-GUARD — `tools/architecture-lint` rejects `prometheus`
  and `actix_web_prom` imports from domain and inbound, and rejects
  `prometheus` outside `outbound::metrics` and outbound adapter tests. Method:
  extend the lint's forbidden-roots table; add a red fixture case to its
  existing `rstest`/BDD suites (`tools/architecture-lint/src/tests.rs`,
  `tests/architecture_guardrails_bdd.rs`) showing an inbound file importing
  `prometheus` is rejected. Rationale: the lint already models per-layer
  forbidden crates; this is the cheapest durable guard against pattern
  regression. Non-vacuity: the new negative fixture must fail against the
  pre-change lint table (asserting the violation is reported for the intended
  layer and crate root), and the whole backend tree must pass the extended lint.

No other non-trivial invariants are introduced: the change moves an existing
counter behind a trait without altering concurrency, ordering, or persisted
state.

Final revalidation of the audit repair (2026-10-16): all deterministic gates,
`make audit`, the metrics-off backend check, and CodeScene passed on the same
dirty candidate. Exact logs and results are recorded in `Progress` and the
private delivery ledger. The lock-only repair still needs commit-bound
CodeRabbit review and hosted CI; these do not alter the implementation
acceptance obligations above.

## Plan of work

Stage A (done): reconnaissance, this document.

Stage B/C per milestone, red tests before production code throughout.

EP-M1. Create `backend/src/domain/ports/pagination_error_metrics.rs`: move
`PaginationErrorSource` here (now `pub`), define the port, and a `NoOp`
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
mirroring `prometheus_idempotency.rs`:
`PrometheusPaginationErrorMetrics::
new(registry: &Registry) -> Result<Self, prometheus::Error>`
building the `IntCounterVec` with the exact `Opts` and label order from the
current `register_pagination_error_metrics`; `increment` calls
`with_label_values([detail_code, source.as_str()]).inc()` (preserve this
argument order — labels are declared `["code", "source"]`). Register in
`backend/src/outbound/metrics/mod.rs` behind the same feature gating as its
siblings. Red: the V-BEHAVIOUR-SAME parameterized tests. Green: implement.

EP-M3. Wiring and deletion, one coherent change:

- Add the handle to inbound state: extend `HttpState` (found via
  `backend/src/inbound/http/users.rs:219`) with
  `pagination_error_metrics: Arc<dyn PaginationErrorMetrics>`; pass it from
  `list_users` through `parse_users_page_params` into `map_cursor_error`
  (signatures gain one `&dyn PaginationErrorMetrics` parameter; callers are all
  in `backend/src/inbound/http/`).
- Outbound: give the Diesel adapters that call
  `map_user_persistence_error` an `Arc<dyn PaginationErrorMetrics>` field, set
  at construction in `backend/src/server/mod.rs`; the mapping functions in
  `user_persistence_error_mapping.rs` gain a `&dyn PaginationErrorMetrics`
  parameter.
- Composition: in `backend/src/server/mod.rs`, select
  `PrometheusPaginationErrorMetrics::new(&prom.registry)` when metrics are
  enabled, else `NoOpPaginationErrorMetrics`, alongside the existing
  idempotency selection. Remove the
  `backend::register_pagination_error_metrics(&metrics.registry)?` call from
  `backend/src/main.rs` and the re-export from `backend/src/lib.rs`.
- Delete `backend/src/observability/pagination_errors.rs` (and the
  `observability` module if empty apart from it; check
  `backend/src/observability/` for siblings first —
  `leta files backend/src/observability`).
- Replace the tests' reliance on the process-global recorder with injected
  recorder assertions. The existing BDD scenario continues to verify the
  invalid-cursor response; it does not assert on the metric counter.

EP-M4. Delete `backend/src/domain/ports/route_metrics.rs` and its `pub use` in
`ports/mod.rs:170`. Update `docs/wildside-backend-architecture.md`: drop or
annotate the `RouteMetrics` sketch so the document no longer describes a trait
the code does not have; note that a route-cache metrics port will be
reintroduced with the cache itself.

EP-M5. Documentation and guardrail:

- Add a "Metrics conventions" subsection to `docs/developers-guide.md`
  (near the existing route-queue metrics section at ~line 1266): one pattern,
  port in `domain::ports`, `NoOp` default, Prometheus adapter in
  `outbound::metrics`, injection at the composition root, no global metric
  state.
- Extend `tools/architecture-lint` per V-LINT-GUARD (red fixture first).
- Update `docs/wildside-backend-architecture.md` Observability section to
  describe pagination-error metrics in their new home.

Delegate mechanical documentation edits to `scribe`; run gates via `scrutineer`
after each milestone.

## Milestones and plateaus

- EP-M1 — port and `NoOp` exist and are tested; no caller changed.
  Acceptance: `make test` green; V-LOG-ALWAYS discharged. Conformance: port
  lives in `domain::ports` (ARCH-PORTS-HOME); no new public interface beyond
  the crate-internal port. Recovery: revert the two new files. Remaining:
  Pattern B still live.
- EP-M2 — Prometheus adapter exists and is tested against a fresh registry;
  still uncalled. Acceptance: V-BEHAVIOUR-SAME discharged. Recovery: revert.
  Remaining: wiring.
- EP-M3 — single pattern in force; global deleted. Acceptance:
  V-GLOBAL-GONE, V-FEATURE-OFF, full gates green, `/metrics` still exposes
  `wildside_pagination_errors_total` (automated check starts the server with
  metrics enabled, triggers an invalid cursor on the users list endpoint, and
  reads `/metrics`). Conformance: no persisted/wire format change (metric name
  and labels preserved). Recovery: this milestone is a single commit; revert
  restores Pattern B intact. Compatibility decision: none —
  `register_pagination_error_metrics` is application-internal and pre-1.0; no
  shim.
- EP-M4 — dead port removed; architecture doc consistent with code.
  Acceptance: V-DEAD-PORT-GONE; `make markdownlint` and `make nixie` green.
  Recovery: revert.
- EP-M5 — convention documented; lint guard active. Acceptance:
  V-LINT-GUARD; docs gates green. Recovery: revert.

Each milestone is committed separately with gates run beforehand.

## Concrete steps

All commands from the repository root. Gate runs are delegated to `scrutineer`;
when run directly, use `tee`:

```bash
make test 2>&1 | tee "/tmp/test-wildside-feat-competing-metrics-patterns.out"
make lint 2>&1 | tee "/tmp/lint-wildside-feat-competing-metrics-patterns.out"
make check-fmt 2>&1 | tee "/tmp/fmt-wildside-feat-competing-metrics-patterns.out"
make markdownlint 2>&1 | tee "/tmp/mdlint-wildside-feat-competing-metrics-patterns.out"
make nixie 2>&1 | tee "/tmp/nixie-wildside-feat-competing-metrics-patterns.out"
```

Note: `make check-fmt`/`make lint` can exit 0 despite sub-check failures — read
the log, not the exit code. The local Whitaker Dylint suite (0.2.7) is ahead of
CI's pin (0.2.6), so `make lint` may be red on a clean tree; compare against a
pre-change baseline run before attributing failures to this work.

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
  recorded here; `make test`, `make lint` (against baseline), `make check-fmt`,
  `make markdownlint`, `make nixie`, and `make audit` green; behavioural check
  of `/metrics` performed at EP-M3; docs updated; five implementation
  milestones committed with imperative, wrapped messages; post-rebase CodeScene
  file/delta checks and implementation review clear. Review-fix and
  plan-closure commits are in Progress. The later lock-only audit repair also
  passes the full local gate sequence; its commit-bound review and hosted CI
  remain delivery steps.

## Idempotence and recovery

Every milestone is a self-contained commit; `git revert` of any single
milestone restores the previous plateau. No data migrations, no persisted
state. Gate log files under `/tmp` are scratch and may be deleted.

## Artefacts and notes

Evidence gathered during reconnaissance (2026-08-16) and preflight (2026-09-28):

- Pattern A: `backend/src/domain/ports/idempotency_metrics.rs:38`
  (`trait IdempotencyMetrics`, `NoOpIdempotencyMetrics` at :62),
  `route_queue_metrics.rs:29`, `enrichment_job_metrics.rs:63` (with
  `mockall::automock`); adapters
  `backend/src/outbound/metrics/prometheus_idempotency.rs:36`,
  `prometheus_route_queue.rs:37`, `prometheus_enrichment_jobs.rs:18`; selection
  in `backend/src/server/mod.rs:98–133`.
- Pattern B: `backend/src/observability/pagination_errors.rs:21`
  (`OnceLock` static), `:42` (`record_pagination_error`), `:67`
  (`register_pagination_error_metrics`); call sites
  `backend/src/inbound/http/users_pagination.rs:286,295` and
  `backend/src/outbound/persistence/user_persistence_error_mapping.rs:26,35`;
  crate-root re-export `backend/src/lib.rs:47`; start-up call
  `backend/src/main.rs:31`.
- Dead port: `backend/src/domain/ports/route_metrics.rs:15`; the source had no
  implementations or callers beyond the re-export at `ports/mod.rs:170`. An
  M4-wide search also found historical references in a completed roadmap
  checklist and the 2025 idempotency-metrics decision log.
- Current fan-out: `map_user_persistence_error` has three adapter callers:
  `diesel_login_service.rs`, `diesel_users_query.rs`, and
  `diesel_user_profile_query.rs`. The HTTP state composition helper is
  `server/state_builders.rs::build_http_state`.
- Open-source tooling check: Firecrawl fetched the Prometheus 0.14.0 Rust
  documentation. `IntCounterVec::with_label_values` matches values positionally
  to declared label names, and `Registry::register` accepts a boxed collector
  and returns an error for invalid or duplicate registrations. References:
  <https://docs.rs/prometheus/0.14.0/prometheus/type.IntCounterVec.html> and
  <https://docs.rs/prometheus/0.14.0/prometheus/struct.Registry.html>.

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
  `backend/src/domain/ports/route_metrics.rs`, the `lib.rs` re-export, and the
  `main.rs` registration call.

## Revision note

On 2026-09-28, preflight found a third persistence mapping caller and the actual
`build_http_state` composition helper. User approvals raised the path cap from
20 to 25, 29 for M5, then 30 for roadmap closeout; the 600-net-line limit and
metrics design and verification obligations remain unchanged.
