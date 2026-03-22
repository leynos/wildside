# Close roadmap item 3.5.4 with revision-safe interests updates

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises &
Discoveries`, `Decision Log`, and `Outcomes & Retrospective` must be kept up
to date as work proceeds.

Status: COMPLETE

## Purpose / big picture

Roadmap item 3.5.4 closes only when `PUT /api/v1/users/me/interests` behaves
like the rest of the revisioned user-state surface: callers can submit an
optional `expectedRevision`, successful writes return the new revision, stale
writes are rejected with a stable conflict envelope, and the persistence logic
stays behind domain ports rather than leaking Diesel or schema details into the
domain or HTTP layers.

The repository already contains a candidate 3.5.4 implementation footprint in
the domain port, Diesel adapter, HTTP handler, behavioural tests, and
architecture document. This plan treats that footprint as provisional. The
team must verify the contract, correct any gaps, prove the behaviour through
`rstest` and `rstest-bdd` (behaviour-driven development, BDD), update the
architecture record if the final decision differs from the current text, run
the full gates, and only then mark roadmap item 3.5.4 as done.

Observable success means all of the following are true:

- `PUT /api/v1/users/me/interests` accepts `expectedRevision` and returns a
  `revision` in the success payload.
- First-write and existing-row semantics are consistent with the shared
  `user_preferences.revision` optimistic-concurrency model.
- Stale writes return HTTP `409 Conflict` with a stable conflict body that
  callers can inspect.
- Interests-only updates preserve non-interest fields stored in
  `user_preferences`.
- Focused unit coverage uses `rstest`, behavioural coverage uses
  `rstest-bdd`, and DB-backed scenarios run through `pg-embedded-setup-unpriv`.
- `docs/wildside-backend-architecture.md` records the final concurrency and
  aggregate-ownership decision.
- `docs/backend-roadmap.md` marks `3.5.4` as done only after every required
  gate passes.
- `make check-fmt`, `make lint`, and `make test` succeed with `tee`-captured
  logs.

## Context and orientation

The current repository state already points to the intended design. The
relevant files are:

- `backend/src/domain/ports/user_interests_command.rs`
- `backend/src/domain/user_interests.rs`
- `backend/src/outbound/persistence/diesel_user_interests_command.rs`
- `backend/src/inbound/http/users.rs`
- `backend/tests/user_interests_revision_conflicts_bdd.rs`
- `backend/tests/features/user_interests_revision_conflicts.feature`
- `docs/wildside-backend-architecture.md`
- `docs/backend-roadmap.md`

At the time this draft was written, the code already showed these candidate
choices:

- the interests driving port uses `UpdateUserInterestsRequest` with
  `expected_revision`;
- the domain success value `UserInterests` includes `revision`;
- the Diesel adapter updates `user_preferences` rather than a separate
  interests table;
- the HTTP handler accepts `expectedRevision`;
- the architecture document already describes interests and preferences as a
  shared aggregate with one revision counter;
- the roadmap entry for `3.5.4` is still unchecked.

That combination means the likely work is not greenfield implementation. It is
an audit-and-close exercise: verify the current code against the roadmap and
docs, repair anything that does not hold up, collect test evidence, and update
the roadmap when the feature is truly complete.

## Agent team and ownership

This plan uses an explicit agent team so each boundary has one owner and the
coordination rules remain clear.

1. Coordinator: owns this ExecPlan, keeps the living sections current, gathers
   evidence, decides whether the current repository state is already compliant
   or needs further edits, and is the only role allowed to mark roadmap item
   `3.5.4` as done.
2. Architecture agent: validates the final design against the hexagonal
   boundary rules in `docs/wildside-backend-architecture.md`, confirms the
   aggregate boundary around `user_preferences`, and records any new or changed
   design decision in the backend architecture document.
3. Persistence agent: owns
   `backend/src/outbound/persistence/diesel_user_interests_command.rs` and its
   tests, with responsibility for the persistence contract, revision checks,
   stale-write detection, and error translation from repository errors to
   domain errors.
4. HTTP contract agent: owns `backend/src/inbound/http/users.rs` and any
   schema-facing tests, ensuring the handler stays thin, request parsing is
   strict, OpenAPI-visible shapes stay correct, and conflict errors are mapped
   consistently.
5. Verification agent: owns `rstest` and `rstest-bdd` coverage, including
   DB-backed flows under `pg-embedded-setup-unpriv`, and captures all targeted
   and full-gate logs with `tee`.

Execution rule: the coordinator may let the architecture and verification
agents work first to freeze the desired contract, then allow the persistence
and HTTP agents to patch gaps in parallel if the audit finds them. The roadmap
and final ExecPlan status must not change until the verification agent has
produced green evidence for the required gates.

## Constraints

- Scope is roadmap item `3.5.4` only. Do not fold `3.5.5` or `3.5.6` into
  this change.
- Preserve hexagonal boundaries:
  - domain defines the interests write contract and the meaning of a
    stale-write conflict;
  - outbound adapters implement persistence and translate persistence errors;
  - inbound HTTP code parses requests, invokes ports, and maps responses.
- Treat `user_preferences` as the canonical persistence model for interests and
  revision tracking. Do not introduce a second aggregate or a second revision
  source.
- Do not add a new migration unless the repository state proves the existing
  schema audit wrong. If that happens, stop and escalate because it contradicts
  roadmap item `3.5.1`.
- Preserve session enforcement and the stable error-envelope shape used by the
  rest of the user-state HTTP surface.
- Use `rstest` for unit-level coverage and `rstest-bdd` for behaviour-level
  coverage.
- Use `pg-embedded-setup-unpriv` for DB-backed local tests.
- Do not add new external dependencies.
- Keep documentation in en-GB-oxendict style.
- Mark `docs/backend-roadmap.md` only after the full gates are green.

## Tolerances

- Scope tolerance: if the work spreads beyond the files listed in the
  orientation section plus directly related tests and docs, stop and record why
  the roadmap item is larger than expected.
- Contract tolerance: if closing `3.5.4` requires changing another public
  endpoint besides `PUT /api/v1/users/me/interests`, stop and escalate.
- Aggregate tolerance: if the final design cannot keep
  `user_preferences.revision` as the single concurrency source, stop and
  escalate.
- Test tolerance: if the verification agent cannot get focused tests and full
  gates green after three disciplined repair loops, stop and capture the best
  evidence rather than guessing.
- Environment tolerance: if embedded PostgreSQL fails before scenario logic
  runs, and the failure is caused by the environment rather than product code,
  stop after capturing logs and do not mark the roadmap item done.
- Tooling tolerance: if `make check-fmt`, `make lint`, or `make test` fail for
  reasons unrelated to this feature, capture the failure, repair the
  environment if safe, and only continue once the root cause is understood.

## Risks

- Risk: interests writes are a partial update of the broader
  `user_preferences` aggregate, so they can conflict with full preferences
  writes even when only interest IDs changed.
  Mitigation: keep one shared revision counter and prove the shared aggregate
  contract in both unit and behavioural tests.

- Risk: the repository already contains a partially finished implementation, so
  stale plan text or stale assumptions may be more dangerous than missing code.
  Mitigation: audit the live files first and let the plan follow the codebase,
  not the older narrative.

- Risk: behavioural tests depend on embedded PostgreSQL and have historical
  bootstrap failures involving `/dev/null` and worker setup.
  Mitigation: run an explicit environment preflight before treating any BDD
  failure as a product regression.

- Risk: handler, schema, adapter, and documentation drift can leave the
  feature "implemented" in code but not safely consumable by clients.
  Mitigation: require one verification pass that checks code, tests, OpenAPI-
  visible shapes, docs, and roadmap state together.

## Implementation plan

### Milestone 1. Audit the current repository state and capture a baseline

The coordinator and architecture agent begin by confirming what is already
implemented and where the remaining gap actually is. This is the moment to
replace assumptions with evidence.

1. Read the live code in the files listed in the orientation section and note
   whether each layer already matches the roadmap contract.
2. Confirm that the architecture document and the code agree on the aggregate
   boundary and the stale-write semantics.
3. Run the environment preflight and targeted tests below, capturing logs even
   if the implementation already looks complete.

```bash
set -o pipefail && ls -l /dev/null 2>&1 \
  | tee /tmp/backend-3-5-4-dev-null.log
set -o pipefail && make prepare-pg-worker 2>&1 \
  | tee /tmp/backend-3-5-4-prepare-pg-worker.log
set -o pipefail && cargo test -p backend user_interests --lib 2>&1 \
  | tee /tmp/backend-3-5-4-targeted-unit.log
set -o pipefail && cargo test -p backend \
  --test user_interests_revision_conflicts_bdd 2>&1 \
  | tee /tmp/backend-3-5-4-targeted-bdd.log
```

Expected evidence:

```plaintext
/dev/null is a character device
targeted unit tests pass or fail with feature-relevant assertions
BDD scenarios either run to scenario assertions
or fail during environment bootstrap
```

If the targeted tests already pass, do not rewrite working code to manufacture
a red phase. Record that the repository already contains a candidate solution
and move to the compliance audit in later milestones. If the tests fail in a
feature-relevant way, keep the logs as the red baseline.

### Milestone 2. Freeze the domain and persistence contract

The architecture and persistence agents own this milestone.

The final contract must preserve one aggregate and one revision source. The
driving port should express what the caller knows, and the outbound adapter
should implement the revision check without leaking Diesel types or SQL details
through the domain boundary.

The required semantic matrix is:

- no existing `user_preferences` row and no `expectedRevision`: create the row
  with revision `1`;
- no existing row and a supplied `expectedRevision`: return a stale-write
  conflict because the caller expected a different aggregate state than exists;
- existing row and no `expectedRevision`: return a conflict because updates to
  an existing aggregate must be revision-safe;
- existing row and matching `expectedRevision`: persist the new interests,
  preserve non-interest fields, and bump the shared revision exactly once;
- existing row and stale `expectedRevision`: return a conflict that surfaces
  the caller's expected revision and the persisted revision.

If the current code already matches this matrix, the persistence agent should
limit edits to missing tests, comments, or edge-case fixes. If it does not
match, correct the live implementation in:

- `backend/src/domain/ports/user_interests_command.rs`
- `backend/src/domain/user_interests.rs`
- `backend/src/outbound/persistence/diesel_user_interests_command.rs`
- related tests under
  `backend/src/outbound/persistence/diesel_user_interests_command/tests/`

Acceptance for this milestone is a clear statement in the ExecPlan that the
contract is frozen and that any remaining work is adapter polish, HTTP polish,
or verification.

### Milestone 3. Verify the HTTP boundary and client-visible error mapping

The HTTP contract agent owns this milestone.

`backend/src/inbound/http/users.rs` must remain a thin adapter. It should parse
`expectedRevision`, validate input IDs, call the driving port, and return the
domain success payload or a stable error envelope. It must not contain
repository-specific branching or Diesel-specific error knowledge.

Review and, if needed, patch the following:

- `InterestsRequest` includes `expected_revision` and serializes cleanly when
  omitted;
- success responses expose `revision`;
- stale writes surface the documented `409 Conflict` envelope;
- handler-level tests exercise both request parsing and error mapping.

If the OpenAPI schema or handler tests do not already cover the `revision`
field and `expectedRevision` semantics, add that coverage before moving on.

### Milestone 4. Prove the behaviour with unit and behavioural tests

The verification agent owns this milestone, with support from the persistence
and HTTP agents when failures reveal real product gaps.

Unit coverage must use `rstest` and should focus on the contract edges that
are easiest to regress:

- first write creates revision `1`;
- matching revision increments exactly once;
- stale revision returns a conflict;
- missing revision on an existing row returns a conflict;
- non-interest fields survive an interests-only update;
- fixture or helper implementations keep pace with the real port contract.

Behavioural coverage must use `rstest-bdd` and the feature file under
`backend/tests/features/user_interests_revision_conflicts.feature`. The DB-
backed scenarios must run through the embedded-Postgres helpers and verify
observable HTTP results rather than internal adapter details.

Run targeted verification with logs:

```bash
set -o pipefail && cargo test -p backend user_interests --lib 2>&1 \
  | tee /tmp/backend-3-5-4-rstest.log
set -o pipefail && cargo test -p backend \
  --test user_interests_revision_conflicts_bdd 2>&1 \
  | tee /tmp/backend-3-5-4-rstest-bdd.log
```

Expected evidence:

```plaintext
test ...fixture_interests_command_echoes_payload ... ok
test ...preserves_non_interest_fields... ok
test ...first_interests_write_creates_revision_1 ... ok
test ...stale_expected_revision_returns_a_conflict ... ok
```

If the BDD suite fails before scenario logic because embedded PostgreSQL cannot
start, treat that as an environment blocker and capture the evidence in the
living sections. Do not mark the roadmap item done until the DB-backed proof is
real.

### Milestone 5. Reconcile the architecture document and close the roadmap item

The architecture agent and coordinator own this milestone.

Audit `docs/wildside-backend-architecture.md` after the code and tests settle.
If the live implementation matches the document already, retain the existing
text and simply note that the document remains accurate. If the live code
forced a different choice, update the architecture document so it records the
final decision rather than the intended one.

Only after the full gates are green may the coordinator:

1. update `docs/backend-roadmap.md` to mark `3.5.4` as done;
2. record the closure state in this ExecPlan, noting that its status is
   already `COMPLETE` and that `Outcomes & Retrospective` captures the actual
   result and any follow-on concerns.

## Validation and gate execution

The verification agent runs all required gates with `tee` and `set -o
pipefail`, keeping the logs for review. These are the minimum closure commands
for this roadmap item:

```bash
set -o pipefail && make check-fmt 2>&1 \
  | tee /tmp/backend-3-5-4-check-fmt.log
set -o pipefail && make lint 2>&1 \
  | tee /tmp/backend-3-5-4-lint.log
set -o pipefail && make test 2>&1 \
  | tee /tmp/backend-3-5-4-test.log
```

If documentation changes are made during execution, also run the documentation
gates that apply in this repository:

```bash
set -o pipefail && make markdownlint 2>&1 \
  | tee /tmp/backend-3-5-4-markdownlint.log
set -o pipefail && make nixie 2>&1 \
  | tee /tmp/backend-3-5-4-nixie.log
```

Gate acceptance is simple:

- every command exits `0`;
- targeted and full logs do not show skipped acceptance criteria being treated
  as success;
- the roadmap entry is updated only after the gate logs exist.

## Approval gate

The approval gate was satisfied on 20 March 2026 before implementation closure
work began. This section is retained as part of the execution record; the
document is no longer a draft.

## Progress

- [x] (2026-03-20 00:00Z) Reviewed the roadmap item, the current backend
  architecture text, the testing guidance, and the live repository state.
- [x] (2026-03-20 00:00Z) Replaced the stale 3.5.4 ExecPlan with this draft.
- [x] (2026-03-20 23:55Z) Approval gate obtained from the user.
- [x] (2026-03-20 23:55Z) Baseline and environment preflight captured.
- [x] (2026-03-21 00:02Z) Domain and persistence contract confirmed.
- [x] (2026-03-21 00:02Z) HTTP contract and schema verified.
- [x] (2026-03-21 00:08Z) `rstest` and `rstest-bdd` evidence captured.
- [x] (2026-03-21 00:09Z) `docs/wildside-backend-architecture.md`
  confirmed accurate and updated to remove stale 3.5.4 deferral text.
- [x] (2026-03-21 00:32Z) `make check-fmt`, `make lint`, and `make test`
  passed.
- [x] (2026-03-21 00:33Z) `docs/backend-roadmap.md` marks `3.5.4` as done.

## Surprises & Discoveries

- The repository already contains a candidate 3.5.4 implementation footprint,
  including port, domain, adapter, BDD, and architecture-document changes,
  while the roadmap entry remains open.
- The previous `docs/execplans/backend-3-5-4-revision-safe-interests-update-
  strategy.md` was no longer a draft and no longer matched the repository
  state, so this plan had to be rewritten as a fresh audit-and-close document.
- Historical notes show that DB-backed verification can fail because of
  environment issues around `/dev/null` and embedded PostgreSQL worker setup,
  so the plan must treat environment preflight as first-class work rather than
  an afterthought.
- Direct `cargo test` invocation for the DB-backed interests BDD does not pick
  up `PG_EMBEDDED_WORKER` automatically; using the Makefile wiring or exporting
  the variable explicitly is required for the suite to exercise product logic
  instead of failing during bootstrap.

## Decision Log

- Decision: treat the current repository state as provisional until the
  verification evidence proves the roadmap acceptance criteria.
  Rationale: code that looks complete but is not fully verified should not
  close the roadmap item.
  Date/Author: 2026-03-20 / planning team.

- Decision: use `user_preferences` as the single aggregate and revision source
  for interests updates.
  Rationale: this preserves the hexagonal model already described in the
  architecture document and avoids inventing a second persistence contract for
  one subset of the same aggregate.
  Date/Author: 2026-03-20 / planning team.

- Decision: use an explicit agent team with one coordinator.
  Rationale: the work crosses domain, persistence, HTTP, tests, and docs, and
  the repository already contains partial implementation that must be audited
  carefully.
  Date/Author: 2026-03-20 / planning team.

- Decision: update the roadmap only after the full gates pass.
  Rationale: the roadmap is a delivery record, not an intent record.
  Date/Author: 2026-03-20 / planning team.

## Outcomes & Retrospective

Roadmap item `3.5.4` is complete. The repository already contained the core
implementation for revision-safe interests updates, and execution work in this
turn verified that implementation against the acceptance criteria and closed
the remaining documentation gap.

Final behaviour confirmed:

- `PUT /api/v1/users/me/interests` accepts optional `expectedRevision`.
- Successful interests writes return the updated shared aggregate `revision`.
- Existing `user_preferences` rows require `expectedRevision`; stale or missing
  revisions surface conflict details with `expectedRevision` and
  `actualRevision`.
- Interests-only updates preserve non-interest preferences fields while using
  the shared `user_preferences.revision` optimistic-concurrency contract.

Evidence captured:

- Targeted unit tests:
  `/tmp/backend-3-5-4-targeted-unit.log`
- Targeted BDD tests:
  `/tmp/backend-3-5-4-targeted-bdd.log`
- Environment preflight:
  `/tmp/backend-3-5-4-dev-null.log`,
  `/tmp/backend-3-5-4-prepare-pg-worker.log`
- Documentation and gate logs:
  `/tmp/backend-3-5-4-markdownlint.log`,
  `/tmp/backend-3-5-4-nixie.log`,
  `/tmp/backend-3-5-4-check-fmt.log`,
  `/tmp/backend-3-5-4-lint.log`,
  `/tmp/backend-3-5-4-test.log`

Environment notes:

- `/dev/null` had drifted back to a regular file and had to be restored to a
  character device before DB-backed verification.
- `yamllint` and `actionlint` were absent and were reinstalled so `make lint`
  could complete.

Follow-on work remains unchanged:

- `3.5.5` should continue to harden startup-mode composition and helper seams.
- `3.5.6` should continue to expand the startup-matrix regression coverage, but
  no additional work is required to close `3.5.4`.
