# Validate and close the Apalis-backed `RouteQueue` adapter (backend 5.2.1)

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

This plan covers roadmap item 5.2.1 only:
`Implement RouteQueue using Apalis with PostgreSQL backend, replacing the
current stub adapter.`

No implementation work may begin from this plan until the plan is explicitly
approved. Approval authorizes the implementation and closure milestones below;
it does not authorize later queue tasks such as job-struct modelling, retry
policy, trace propagation, worker deployment, or route-submission dispatch.

## Purpose / big picture

Wildside's route generation work needs a real queue adapter so route jobs can
be persisted for background processing instead of disappearing through a stub.
The domain already owns the queue contract through the `RouteQueue` port, and
the adapter boundary must keep Apalis, SQLx, and PostgreSQL details out of the
domain.

The repository currently contains an Apalis/PostgreSQL queue adapter and tests
from prior work, but `docs/backend-roadmap.md` still lists 5.2.1 as open and
the existing plan history is stale. This plan therefore treats 5.2.1 as a
validation-and-closure activity: confirm the adapter still satisfies the
roadmap item on the current base, repair any defects found within the
tolerances below, run the required gates, record CodeRabbit review outcomes,
and mark the roadmap item done only after evidence is clean.

Observable success means:

- `backend::outbound::queue::ApalisRouteQueue` persists queue payloads through
  Apalis PostgreSQL storage.
- `backend::domain::ports::RouteQueue` remains the only domain-facing queue
  contract.
- Unit tests using `rstest` cover happy and unhappy adapter paths.
- Behavioural tests using `rstest-bdd` cover PostgreSQL-backed queue
  persistence and failure behaviour.
- Documentation accurately explains the adapter scope, the SQLx pool used by
  Apalis, and the fact that worker consumption remains future work.
- `make check-fmt`, `make lint`, and `make test` pass.
- CodeRabbit review has been run after each major milestone, and all concerns
  have either been fixed or explicitly documented as not applicable.
- `docs/backend-roadmap.md` marks 5.2.1 done only after the evidence above is
  available.

## Constraints

- Do not start implementation until this DRAFT plan has explicit approval.
- Keep scope to backend roadmap item 5.2.1. Do not implement or mark done
  5.2.2, 5.2.3, 5.2.4, 5.3.1, or any route-submission dispatch item.
- Preserve hexagonal architecture. Domain and inbound modules must not import
  Apalis, SQLx, or concrete outbound queue types.
- Keep `RouteQueue` and `JobDispatchError` domain-owned. Do not widen the port
  just to expose Apalis details.
- Keep worker consumption out of scope. No `WorkerBuilder`, `Monitor`, retry
  policy, dead-letter queue, or worker deployment work belongs in this plan.
- Keep request-path dispatch out of scope unless the plan is revised and
  re-approved. The current `TODO(#276)` queue-dispatch markers in
  `backend/src/domain/route_submission/mod.rs` remain later work.
- Retain `StubRouteQueue` for tests and non-PostgreSQL development paths if it
  is still used.
- Use `apalis-postgres` with PostgreSQL storage as required by the roadmap.
- Use `rstest` for focused unit tests and `rstest-bdd` for behavioural tests.
- Use the existing `pg-embedded-setup-unpriv` test infrastructure for live
  PostgreSQL tests.
- Keep source files below 400 lines, or split them before completing the
  milestone.
- Keep documentation in en-GB-oxendict style and follow
  `docs/documentation-style-guide.md`.
- Prefer Makefile gates over raw tool invocations for final validation.
- Run tests, formatting, and linting sequentially, not in parallel.
- Capture long command output with `tee` under `/tmp`.
- Commit each approved implementation milestone only after its gate passes.
- Do not mark the roadmap item done until all required gates and CodeRabbit
  review concerns are clear.

## Tolerances (exception triggers)

- Scope: stop and escalate if satisfying 5.2.1 requires changing more than 12
  production source files or more than 800 net lines outside tests and
  documentation.
- Port shape: stop and escalate before changing the `RouteQueue` trait
  signature or adding Apalis-specific variants to `JobDispatchError`.
- Runtime wiring: stop and escalate before wiring `RouteQueue` into
  `RouteSubmissionServiceImpl`, HTTP handlers, or server startup.
- Dependencies: `apalis-core`, `apalis-postgres`, and `sqlx` are expected. Stop
  and escalate if more than two additional production dependencies are needed.
- Persistence schema: stop and escalate if Apalis table setup conflicts with
  Diesel migrations or requires Diesel-managed Apalis migrations.
- Test harness: stop and document the blocker if embedded PostgreSQL cannot be
  started with the existing `pg-embedded-setup-unpriv` flow.
- Verification: stop and document logs if `make check-fmt`, `make lint`, or
  `make test` still fail after three focused repair loops.
- CodeRabbit: stop and document the concern if `coderabbit review --agent`
  reports a finding that would require widening the approved scope.

## Risks

- Risk: the current base already contains an Apalis adapter, so the remaining
  work may be partly reconciliation rather than new implementation.
  Mitigation: start by auditing the current symbols, tests, dependencies, and
  docs. If the adapter is already correct, make only closure changes such as
  roadmap updates and plan evidence.

- Risk: Apalis release-candidate APIs may have shifted. Firecrawl research on
  2026-05-21 found `apalis-postgres` latest documentation at 1.0.0-rc.8, while
  this repository currently pins older 1.0.0 release-candidate crates.
  Mitigation: do not upgrade during 5.2.1 unless a gate failure requires it.
  Record any version decision in this plan before changing dependencies.

- Risk: Apalis uses SQLx while repository adapters use Diesel and `bb8`.
  Mitigation: keep the Apalis SQLx `PgPool` contained in
  `backend/src/outbound/queue/*` and document the dual-pool boundary.

- Risk: `PostgresStorage::setup()` creates Apalis-owned tables that are not
  represented in Diesel migrations.
  Mitigation: verify setup is idempotent in tests and document that Apalis owns
  its internal queue schema.

- Risk: route-submission user flows may appear to require end-to-end queue
  dispatch to satisfy "replacing the current stub adapter".
  Mitigation: keep the acceptance boundary explicit. 5.2.1 closes when the
  driven adapter exists and is validated; dispatch and worker processing are
  later roadmap items.

- Risk: `docs/users-guide.md` was requested, but this workspace does not
  contain that file.
  Mitigation: update `docs/developers-guide.md` and
  `docs/wildside-backend-architecture.md` for internal behaviour. If no
  end-user server behaviour changes, record that `docs/users-guide.md` is
  absent and no user-facing guide update was possible.

## Skills and reference documents

Use the following skills while executing this plan:

- `leta`: navigate symbols and references before editing code.
- `rust-router`: route any Rust language issue to the smallest useful Rust
  skill.
- `hexagonal-architecture`: enforce port and adapter boundaries.
- `rust-async-and-concurrency`: review async ownership, storage handles, and
  worker-adjacent lifecycle implications.
- `rust-errors`: review `JobDispatchError` mapping.
- `domain-cli-and-daemons`: keep future worker process concerns out of this
  adapter milestone.
- `firecrawl-mcp`: verify external Apalis and PostgreSQL queue-tooling facts.
- `commit-message`: commit with a file-based commit message.
- `pr-creation` and `en-gb-oxendict-style`: prepare the draft pull request.

Read these repository documents before implementation:

- `docs/backend-roadmap.md`
- `docs/wildside-backend-architecture.md`
- `docs/developers-guide.md`
- `docs/documentation-style-guide.md`
- `docs/rust-testing-with-rstest-fixtures.md`
- `docs/rstest-bdd-users-guide.md`
- `docs/rust-doctest-dry-guide.md`
- `docs/complexity-antipatterns-and-refactoring-strategies.md`
- `docs/pg-embed-setup-unpriv-users-guide.md`

External references checked during planning:

- `https://docs.rs/apalis-postgres/latest/apalis_postgres/`
- `https://apalis.dev/docs/introduction/quickstart`

## Current repository orientation

The current base contains these relevant files:

- `backend/src/domain/ports/route_queue.rs` defines the domain-owned
  `RouteQueue` trait and `JobDispatchError`.
- `backend/src/outbound/queue/mod.rs` exports queue adapters.
- `backend/src/outbound/queue/stub_route_queue.rs` retains the no-op stub.
- `backend/src/outbound/queue/apalis_route_queue.rs` contains the
  Apalis-backed adapter and its focused unit tests.
- `backend/src/outbound/queue/test_helpers.rs` contains fake queue providers
  for unit tests.
- `backend/tests/features/route_queue_apalis.feature` describes behavioural
  queue scenarios.
- `backend/tests/route_queue_apalis_bdd.rs` implements the behavioural tests.
- `backend/tests/support/embedded_postgres.rs` contains embedded PostgreSQL
  support, including Apalis storage setup helpers.
- `backend/src/domain/route_submission/mod.rs` still has queue-dispatch TODOs
  and must remain out of scope for this plan.
- `backend/src/server/mod.rs` does not currently construct or inject a
  route-queue dependency into route submission.
- `docs/wildside-backend-architecture.md` and `docs/developers-guide.md`
  already mention Apalis queue adapter behaviour and must be checked against
  the implementation before closure.

Key terms:

- Apalis is a Rust background task processing library.
- `apalis-postgres` is the Apalis backend that stores tasks in PostgreSQL.
- `PostgresStorage` is the Apalis storage type used for polling-backed
  PostgreSQL queues.
- `PostgresStorage::setup()` provisions Apalis-owned queue tables.
- A driven adapter is outbound infrastructure code that implements a
  domain-owned port.

## Implementation plan

Milestone 0: approval and baseline audit.

After approval, confirm the branch and workspace:

```bash
git branch --show-current
git status --short --branch
leta workspace add "$(pwd)"
```

The expected branch is `backend-5-2-1-apalis-route-queue`. The working tree
should be clean before implementation begins. If there are user changes, do
not overwrite them.

Use Leta and plain text search for non-code documents to confirm the current
adapter state:

```bash
leta grep "RouteQueue|ApalisRouteQueue|GenericApalisRouteQueue" backend \
  -k trait,struct,enum,function,method --head 200
rg -n "5.2.1|Apalis|RouteQueue|PostgresStorage" \
  docs/backend-roadmap.md docs/wildside-backend-architecture.md \
  docs/developers-guide.md backend/Cargo.toml
```

Record the findings in `Surprises & Discoveries` before editing code.

Milestone 1: targeted adapter verification.

Run the focused queue unit and behavioural suites. These commands intentionally
use Cargo directly because they isolate the feature before the full Makefile
gates.

```bash
set -o pipefail
cargo test -p backend outbound::queue --lib 2>&1 \
  | tee /tmp/backend-5-2-1-queue-unit.out

set -o pipefail
cargo test -p backend --test route_queue_apalis_bdd 2>&1 \
  | tee /tmp/backend-5-2-1-queue-bdd.out
```

If both pass, update `Progress` with the log paths and move to Milestone 2. If
either fails, repair only adapter, test-harness, or documentation defects that
are inside this plan's tolerances. Do not wire request-path dispatch to make
these tests pass.

Run CodeRabbit after this milestone:

```bash
coderabbit review --agent
```

Fix all in-scope concerns before continuing. If CodeRabbit requests
out-of-scope work, document it and escalate.

Commit the passing milestone with a file-based commit message if any files
changed.

Milestone 2: documentation reconciliation.

Review and update only documentation that is stale or inaccurate:

- `docs/wildside-backend-architecture.md` must explain that 5.2.1 covers the
  driven queue adapter, not worker consumption or request dispatch.
- `docs/developers-guide.md` must explain adapter boundaries, dependency
  visibility, and how to run the queue tests. In the approved implementation
  baseline, `GenericApalisRouteQueue<P, Q>` is re-exported for the BDD harness
  seam even though production code should prefer `ApalisRouteQueue<P>`.
- `docs/pg-embed-setup-unpriv-users-guide.md` must be updated only if the
  embedded PostgreSQL setup expectations changed.
- `docs/users-guide.md` must be updated if it exists and user-facing server
  behaviour changed. If the file is still absent and the server interface did
  not change, record that no user-guide update was applicable.

Run documentation checks after documentation edits:

```bash
set -o pipefail
make markdownlint 2>&1 | tee /tmp/backend-5-2-1-markdownlint.out

set -o pipefail
make nixie 2>&1 | tee /tmp/backend-5-2-1-nixie.out
```

Run CodeRabbit again:

```bash
coderabbit review --agent
```

Fix all in-scope concerns before continuing. Commit the passing milestone if
any files changed.

Milestone 3: full quality gates.

Run the required repository gates sequentially:

```bash
set -o pipefail
make check-fmt 2>&1 | tee /tmp/backend-5-2-1-check-fmt.out

set -o pipefail
make lint 2>&1 | tee /tmp/backend-5-2-1-lint.out

set -o pipefail
make test 2>&1 | tee /tmp/backend-5-2-1-test.out
```

If a gate fails, repair only in-scope defects and rerun the failing gate. After
the failing gate passes, rerun any later gates in sequence. Stop after three
repair loops for the same gate.

Run CodeRabbit after full gates:

```bash
coderabbit review --agent
```

Milestone 4: roadmap closure.

Only after Milestones 1 through 3 pass and CodeRabbit concerns are clear,
update `docs/backend-roadmap.md`:

```markdown
- [x] 5.2.1. Implement `RouteQueue` using Apalis with PostgreSQL backend,
  replacing the current stub adapter.
```

Add a short execution note under the item if useful, pointing to the queue
tests and this ExecPlan. Do not mark 5.2.2 or later items done.

Run the final required gates again if the roadmap edit triggers formatting or
lint changes:

```bash
set -o pipefail
make check-fmt 2>&1 | tee /tmp/backend-5-2-1-final-check-fmt.out

set -o pipefail
make lint 2>&1 | tee /tmp/backend-5-2-1-final-lint.out

set -o pipefail
make test 2>&1 | tee /tmp/backend-5-2-1-final-test.out
```

Commit the roadmap closure separately.

Milestone 5: push and pull request.

Push the branch and set upstream tracking:

```bash
git push -u origin backend-5-2-1-apalis-route-queue
```

Create or update a draft pull request. The title must include the roadmap item
as `(backend-5.2.1)`. The body must mention this ExecPlan and include a
`## References` section with the Lody session link from:

```bash
echo "${LODY_SESSION_ID}"
```

The draft pull request for the plan itself must say that implementation awaits
explicit approval. The implementation pull request, if later created from the
approved work, must include gate logs and CodeRabbit outcomes.

## Validation and acceptance

5.2.1 can be marked complete only when all of the following are true:

- `ApalisRouteQueue<P>` implements `RouteQueue<Plan = P>` using
  `apalis-postgres` PostgreSQL storage.
- Queue payload serialization failures map to `JobDispatchError::Rejected`.
- PostgreSQL, SQLx, or Apalis enqueue failures map to
  `JobDispatchError::Unavailable`.
- The domain and inbound layers do not import Apalis or SQLx.
- Request-path dispatch and worker consumption remain out of scope.
- Focused `rstest` unit tests pass.
- `rstest-bdd` queue scenarios pass against embedded PostgreSQL.
- Documentation accurately states the adapter's scope and operational
  boundaries.
- `make check-fmt`, `make lint`, and `make test` pass.
- `coderabbit review --agent` has no unresolved in-scope concerns.
- `docs/backend-roadmap.md` marks only item 5.2.1 done.

`proptest`, `kani`, and `verus` are not required for this milestone unless the
implementation introduces new pure invariants beyond serialization and
infrastructure error mapping. The queue adapter is primarily an external
system integration, so focused unit tests and PostgreSQL-backed behavioural
tests provide the appropriate level of rigour for 5.2.1.

## Idempotence and recovery

The validation commands are safe to rerun. `PostgresStorage::setup()` is
expected to be idempotent. Embedded PostgreSQL tests provision isolated
databases through the existing test harness.

If a command is interrupted, rerun the same command with the same log path or a
new path with an `-attempt-N` suffix. If embedded PostgreSQL setup fails for an
environmental reason, record the exact log path in `Surprises & Discoveries`
and stop rather than replacing the repository's test infrastructure.

No destructive Git commands are required. Do not use `git reset --hard` or
`git checkout --` to discard work unless the user explicitly asks for that
operation.

## Progress

- [x] 2026-05-21: Loaded `leta`, `rust-router`, `hexagonal-architecture`,
  `execplans`, `firecrawl-mcp`, `pr-creation`, and supporting Rust skills for
  planning.
- [x] 2026-05-21: Created the Leta workspace for this worktree.
- [x] 2026-05-21: Renamed the local branch to
  `backend-5-2-1-apalis-route-queue`.
- [x] 2026-05-21: Used a Wyvern planning team to inspect roadmap/docs, current
  queue code, and test coverage.
- [x] 2026-05-21: Used Firecrawl to verify current Apalis/PostgreSQL
  documentation and queue concepts.
- [x] 2026-05-21: Drafted this approval-gated ExecPlan.
- [x] 2026-05-21: Validated the draft with `make check-fmt`
  (`/tmp/check-fmt-wildside-backend-5-2-1-apalis-route-queue.out`).
- [x] 2026-05-21: Validated the draft with `make lint`
  (`/tmp/lint-wildside-backend-5-2-1-apalis-route-queue.out`).
- [x] 2026-05-21: Validated the draft with `make test`
  (`/tmp/test-wildside-backend-5-2-1-apalis-route-queue.out`; 1220 tests
  passed, 4 skipped).
- [x] 2026-05-21: Attempted `coderabbit review --agent` twice for the plan
  milestone; both attempts were blocked by a recoverable service rate limit.
- [x] 2026-05-26: Received explicit approval to implement this ExecPlan.
- [x] 2026-05-26: Confirmed branch
  `backend-5-2-1-apalis-route-queue`, clean worktree, and existing Leta
  workspace.
- [x] 2026-05-26: Completed baseline audit. `ApalisRouteQueue` and
  `GenericApalisRouteQueue` are present under `backend/src/outbound/queue`,
  current documentation describes the Apalis/PostgreSQL boundary, and
  `docs/backend-roadmap.md` still lists 5.2.1 as open.
- [x] 2026-05-26: Ran targeted queue unit tests with
  `cargo test -p backend outbound::queue --lib`
  (`/tmp/backend-5-2-1-queue-unit.out`); 5 passed.
- [x] 2026-05-26: Ran targeted Apalis queue BDD tests with
  `cargo test -p backend --test route_queue_apalis_bdd`
  (`/tmp/backend-5-2-1-queue-bdd.out`); 9 passed.
- [x] 2026-05-26: Ran applicable pre-review gates:
  `make check-fmt` (`/tmp/backend-5-2-1-pre-coderabbit-check-fmt.out`) and
  `make markdownlint`
  (`/tmp/backend-5-2-1-pre-coderabbit-markdownlint.out`); both passed.
- [x] 2026-05-26: Ran `coderabbit review --agent` after targeted adapter
  verification; review completed with 0 findings.
- [x] 2026-05-26: Reconciled `docs/developers-guide.md` with the current
  queue adapter API by documenting that `GenericApalisRouteQueue<P, Q>` is
  re-exported for the BDD harness seam.
- [x] 2026-05-26: Ran documentation checks after documentation
  reconciliation: `make markdownlint` (`/tmp/backend-5-2-1-markdownlint.out`)
  and `make nixie` (`/tmp/backend-5-2-1-nixie.out`); both passed.
- [x] 2026-05-26: Confirmed `docs/users-guide.md` is absent and that 5.2.1
  does not change user-facing server behaviour, so no user-guide update is
  applicable.
- [x] 2026-05-26: Ran `coderabbit review --agent` after documentation
  reconciliation; review completed with 0 findings.
- [x] 2026-05-26: Ran full gates: `make check-fmt`
  (`/tmp/backend-5-2-1-check-fmt.out`), `make lint`
  (`/tmp/backend-5-2-1-lint.out`), and `make test`
  (`/tmp/backend-5-2-1-test.out`); all passed. The Rust nextest portion ran
  1220 tests with 1220 passed and 4 skipped, and the frontend/workspace tests
  also passed.
- [x] 2026-05-26: Ran `coderabbit review --agent` after full gates; review
  completed with 0 findings.
- [x] 2026-05-26: Marked only `docs/backend-roadmap.md` item 5.2.1 done.
- [x] 2026-05-26: Ran final closure gates after the roadmap update:
  `make check-fmt` (`/tmp/backend-5-2-1-final-check-fmt.out`), `make lint`
  (`/tmp/backend-5-2-1-final-lint.out`), and `make test`
  (`/tmp/backend-5-2-1-final-test.out`); all passed. The Rust nextest portion
  ran 1220 tests with 1220 passed and 4 skipped, and the frontend/workspace
  tests also passed.
- [x] 2026-05-26: Ran `coderabbit review --agent` after the final closure
  commit; review completed with 0 findings.
- [x] 2026-05-26: Completed outcomes and retrospective.

## Surprises & discoveries

- 2026-05-21: The current base already contains
  `backend/src/outbound/queue/apalis_route_queue.rs`,
  `backend/tests/route_queue_apalis_bdd.rs`, and Apalis queue documentation.
  The branch started with no diff from `origin/main`, while the roadmap item
  still remained unchecked. This plan therefore focuses on approval-gated
  validation, repair, and closure rather than assuming the adapter must be
  written from scratch.

- 2026-05-21: `docs/users-guide.md` is absent from this worktree. User-visible
  behaviour does not appear to change in 5.2.1 because request-path dispatch
  and workers remain future work; internal docs are the likely documentation
  surface.

- 2026-05-21: CodeRabbit review could not complete for the plan-drafting
  milestone because the service returned a recoverable rate-limit error on two
  attempts. No CodeRabbit concerns were produced. Approved implementation must
  retry CodeRabbit before moving beyond the first implementation milestone.

- 2026-05-26: The implementation baseline still matches the planning audit.
  The adapter exists in the outbound queue module, the domain-facing
  `RouteQueue` port remains the published contract, and request-path dispatch
  remains out of scope.

- 2026-05-26: `docs/developers-guide.md` incorrectly described
  `GenericApalisRouteQueue<P, Q>` as not re-exported. The code re-exports it
  from `backend/src/outbound/queue/mod.rs`, and the Apalis BDD test harness
  imports that re-export directly.

- 2026-05-26: `make nixie` runs `bun install`, which rewrote `bun.lock` to
  bump the `ip-address` override from `10.1.1` to `10.2.0`. That lockfile
  change was unrelated to 5.2.1 and was removed from this milestone.

## Decision Log

- Decision: Treat this plan as an approval-gated validation and closure plan
  for 5.2.1, not as permission to continue the prior in-progress execution.
  Rationale: the user explicitly stated that the plan must be approved before
  implementation, and the branch currently has no diff from `origin/main`.
  Date/Author: 2026-05-21 / planning agent.

- Decision: Keep route-submission dispatch and worker consumption out of
  scope.
  Rationale: roadmap items 5.2.2 through 5.3.1 cover job structs, retries,
  trace propagation, and worker deployment. The current route-submission TODOs
  show dispatch remains deliberately deferred.
  Date/Author: 2026-05-21 / planning agent.

- Decision: Use `apalis-postgres` with PostgreSQL storage for the queue
  adapter.
  Rationale: `docs/backend-roadmap.md` explicitly requires Apalis with a
  PostgreSQL backend. Firecrawl research confirmed Apalis PostgreSQL
  documentation describes `PostgresStorage`, storage setup, standard polling,
  `NOTIFY`-based storage, heartbeat support, and orphaned job re-enqueueing.
  Date/Author: 2026-05-21 / planning agent.

- Decision: Do not upgrade Apalis dependencies as part of the plan draft.
  Rationale: external documentation currently shows a newer release candidate
  than the repository pins, but dependency upgrades are implementation work and
  need evidence from gates before they are justified.
  Date/Author: 2026-05-21 / planning agent.

- Decision: Start approved execution as a validation-and-closure activity.
  Rationale: the user's 2026-05-26 instruction explicitly approves proceeding
  with the planned functionality, and the baseline audit confirms that the
  adapter already exists and the remaining work is to validate, repair any
  defects found, reconcile documentation, and close the roadmap item.
  Date/Author: 2026-05-26 / implementation agent.

- Decision: Keep the current `GenericApalisRouteQueue<P, Q>` re-export and
  document it as a BDD harness seam.
  Rationale: changing visibility would be an API adjustment rather than a
  required 5.2.1 behaviour fix. The re-export is already used by the
  PostgreSQL-backed BDD test, while production code still has the clearer
  `ApalisRouteQueue<P>` alias.
  Date/Author: 2026-05-26 / implementation agent.

## Outcomes & Retrospective

5.2.1 is complete as a validation-and-closure milestone. The current backend
already contained the Apalis/PostgreSQL `RouteQueue` adapter, so the approved
implementation work verified that adapter against focused `rstest` unit tests,
PostgreSQL-backed `rstest-bdd` scenarios, and full repository gates rather
than rewriting working code.

The only documentation defect found was in `docs/developers-guide.md`, which
described `GenericApalisRouteQueue<P, Q>` as internal even though the backend
module re-exports it for the BDD harness seam. That guide now matches the
actual public adapter surface and still directs production code to prefer the
`ApalisRouteQueue<P>` alias.

No user-facing server behaviour changed, and this worktree does not contain
`docs/users-guide.md`, so no user-guide update was applicable. Route-submission
dispatch, worker consumption, retry policy, trace propagation, and route-engine
invocation remain explicitly deferred to later roadmap items.

CodeRabbit returned 0 findings after targeted adapter verification,
documentation reconciliation, full quality gates, and final roadmap closure.
`docs/backend-roadmap.md` now marks only item 5.2.1 as done.
