# Publish crate-level pagination documentation (roadmap 4.1.3)

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

This plan covers roadmap item 4.1.3 only:
`Publish crate-level documentation outlining ordering requirements, default and
maximum limits (20 and 100), and error mapping guidelines.`

## Purpose / big picture

The `pagination` crate at `backend/crates/pagination` already ships the types
needed for keyset pagination — `Cursor<Key>`, `Direction`, `PageParams`,
`Paginated<T>`, and `PaginationLinks` — but the crate-level documentation
(the `//!` block in `lib.rs`) is minimal: a six-line summary, a brief heading
about cursor-based pagination, and a single code example. A developer picking
up the crate for the first time cannot answer critical integration questions
without reading every source file:

- What ordering guarantees must a key type satisfy?
- What are the shared default and maximum page sizes, and why?
- How should adapter or handler code map `CursorError` and `PageParamsError`
  to HTTP status codes?
- What does the crate intentionally *not* provide (Diesel filters,
  Actix extractors, connection pooling)?

After this change, a developer opening the pagination crate's documentation
(via `cargo doc -p pagination --open` or reading `lib.rs`) will find a
self-contained reference covering:

1. **Ordering requirements** — what "total ordering" means in practice, why the
   key type must correspond to a composite database index, and what happens if
   the ordering is not stable.
2. **Default and maximum limits** — the shared constants `DEFAULT_LIMIT` (20)
   and `MAX_LIMIT` (100), how `PageParams` enforces them, and what callers
   should expect when a limit is omitted, zero, or too large.
3. **Error mapping guidelines** — how `CursorError` variants
   (`InvalidBase64`, `Deserialize`, `Serialize`) and `PageParamsError`
   (`InvalidLimit`) should be translated to HTTP responses, with recommended
   status codes and envelope shapes.
4. **Scope boundaries** — an explicit "what this crate does not do" section so
   consumers know which responsibilities belong to inbound or outbound
   adapters.

The change also adds unit tests (via `rstest`) and behavioural tests (via
`rstest-bdd`) that validate the documented invariants hold at runtime:
defaults are applied, limits are capped, zero limits are rejected, and error
variants carry useful messages.

Observable success criteria:

- `cargo doc -p pagination --no-deps` builds without warnings and the
  generated HTML contains sections titled "Ordering requirements", "Default
  and maximum limits", "Error mapping guidelines", and "Scope boundaries".
- Existing tests continue to pass: `cargo test -p pagination` returns zero
  failures.
- New documentation-focused tests confirm the documented invariants: default
  limit is 20, maximum limit is 100, zero limit yields `PageParamsError`,
  and cursor error variants are distinguishable.
- New BDD scenarios confirm the documented behaviour from an integration
  perspective.
- `make check-fmt`, `make lint`, and `make test` all pass with logs retained.
- `docs/backend-roadmap.md` marks item 4.1.3 as done.
- `docs/wildside-backend-architecture.md` records the documentation scope
  decision.

## Constraints

Hard invariants that must hold throughout implementation. Violation requires
escalation, not workarounds.

- Scope is roadmap item 4.1.3 only. Do not mark any other roadmap item done.
- The pagination crate must remain transport- and persistence-neutral. No
  dependencies on Actix, Diesel, or backend domain modules may be introduced.
- The existing public API surface (`Cursor`, `CursorError`, `Direction`,
  `Paginated`, `PaginationLinks`, `PageParams`, `PageParamsError`,
  `DEFAULT_LIMIT`, `MAX_LIMIT`) must not change in any breaking way. This
  task adds documentation and tests, not new types or breaking signatures.
- New documentation must follow en-GB-oxendict spelling as required by
  `AGENTS.md` and `docs/documentation-style-guide.md`.
- Rustdoc examples must be marked `no_run` per the documentation style guide
  (`docs/documentation-style-guide.md`, line 81–82).
- Every module must begin with a module-level (`//!`) comment explaining the
  module's purpose per `AGENTS.md` line 136.
- No single code file may exceed 400 lines per `AGENTS.md` line 32.
- Use `rstest` for unit tests and `rstest-bdd` for behavioural tests per
  `AGENTS.md` lines 173–174.
- Use `pg-embedded-setup-unpriv` for any tests requiring Postgres (not
  expected here, but stated for completeness).
- Preserve hexagonal boundaries: the pagination crate sits outside the
  hexagonal layers and must not import from `backend::domain`,
  `backend::inbound`, or `backend::outbound`.
- New Rustdoc comments must carry `# Examples`, `# Errors`, `# Parameters`,
  and `# Returns` sections as appropriate per
  `docs/documentation-style-guide.md`.

## Tolerances (exception triggers)

Thresholds that trigger escalation when breached. These define the boundaries
of autonomous action, not quality criteria.

- Scope: if implementation requires changes to more than 12 files or 400 lines
  of code (net), stop and escalate.
- Dependencies: if any new production or dev dependency is required beyond what
  the crate already declares, stop and escalate.
- Interface changes: if any public type signature, trait, or constant must
  change to satisfy the documentation requirements, stop and escalate.
- Iterations: if `make check-fmt`, `make lint`, or `make test` still fail
  after three repair loops, stop and capture the failing logs.
- Environment: if embedded Postgres or the Rust toolchain cannot execute
  locally, stop and document the exact blocker with command output.
- Gate: if a quality gate failure is caused by pre-existing issues unrelated
  to this task (e.g. `/dev/null` drift, missing `actionlint`), document it
  and proceed with the feature-scoped gates (`cargo test -p pagination`,
  `cargo clippy -p pagination -- -D warnings`, `cargo fmt -p pagination --
  --check`).

## Risks

Known uncertainties that might affect the plan.

- Risk: the existing crate documentation example in `lib.rs` already
  demonstrates basic usage; expanding it may push `lib.rs` close to the
  400-line limit.
  Severity: low.
  Likelihood: medium.
  Mitigation: keep prose concise, use short focused examples per section, and
  measure line count after each edit.

- Risk: documentation-focused tests may duplicate existing tests that already
  cover default limits and cursor error variants.
  Severity: low.
  Likelihood: high.
  Mitigation: review existing tests first and only add tests that validate
  newly documented invariants not already covered (e.g. verifying that
  documented error messages match reality, or that documented mappings hold).
  Where an existing test already covers a documented invariant, reference it
  in the documentation rather than duplicating.

- Risk: full-gate failures may come from environment drift (broken
  `/dev/null`, missing `yamllint` or `actionlint`) rather than from this
  change.
  Severity: medium.
  Likelihood: medium.
  Mitigation: retain logs with `tee`, rely on `make test` for automatic
  `PG_EMBEDDED_WORKER` wiring, and treat environment failures as separate
  from feature regressions. Use crate-scoped gates as the primary signal.

- Risk: the `docs/documentation-style-guide.md` requires `no_run` on all
  examples, but some existing pagination crate examples do not use `no_run`
  (they run as doctests). Changing them to `no_run` would remove doctest
  coverage.
  Severity: medium.
  Likelihood: high.
  Mitigation: inspect the style guide carefully. The guide says to mark
  examples with `no_run` under the "API doc comments (Rust)" heading. The
  existing crate examples that run as doctests provide value; new examples
  added in this task should follow the style guide. If a conflict arises,
  document the decision to keep runnable doctests for existing examples and
  use `no_run` for new prose-heavy examples.

## Agent team and ownership

This implementation should use an explicit agent team. One person may play
more than one role, but the ownership boundaries should remain visible.

- Coordinator agent:
  Owns sequencing, keeps this ExecPlan current, enforces tolerances, collects
  gate evidence, and decides when roadmap item 4.1.3 is ready to close.

- Documentation agent:
  Owns `backend/crates/pagination/src/lib.rs` crate-level documentation
  expansion, module-level (`//!`) documentation improvements across
  `cursor.rs`, `envelope.rs`, and `params.rs`, and any new Rustdoc examples.

- Quality assurance (QA) agent:
  Owns new `rstest` unit tests and `rstest-bdd` behavioural scenarios that
  validate the documented invariants. Owns the BDD feature file and step
  definitions.

- Architecture agent:
  Owns updates to `docs/wildside-backend-architecture.md` (recording the
  documentation scope decision) and `docs/backend-roadmap.md` (marking 4.1.3
  done).

Hand-off order:

1. Coordinator agent reviews current crate state, confirms scope, and drafts
   this ExecPlan.
2. Documentation agent expands the crate-level and module-level documentation
   with the four required sections.
3. QA agent adds new unit tests and BDD scenarios that verify the documented
   invariants.
4. Documentation agent reviews test assertions against the documentation to
   ensure consistency.
5. Architecture agent records the design decision and closes the roadmap item.
6. Coordinator agent runs final gates and updates this ExecPlan.

## Progress

- [ ] Review current crate documentation, tests, and line counts.
- [ ] Expand `lib.rs` crate-level documentation with four sections: ordering
  requirements, default and maximum limits, error mapping guidelines, and
  scope boundaries.
- [ ] Improve module-level documentation in `cursor.rs`, `envelope.rs`, and
  `params.rs` where the existing `//!` comments are minimal.
- [ ] Add new BDD feature file for documentation invariants.
- [ ] Add step definitions for new BDD scenarios.
- [ ] Add any new `rstest` unit tests for invariants not already covered.
- [ ] Run crate-scoped gates: `cargo test -p pagination`, `cargo clippy -p
  pagination -- -D warnings`, `cargo fmt -p pagination -- --check`.
- [ ] Run full gates: `make check-fmt`, `make lint`, `make test`.
- [ ] Record design decision in `docs/wildside-backend-architecture.md`.
- [ ] Mark roadmap item 4.1.3 done in `docs/backend-roadmap.md`.
- [ ] Update this ExecPlan to COMPLETE status.

## Surprises & discoveries

(None yet — to be filled during implementation.)

## Decision log

(None yet — to be filled during implementation.)

## Outcomes & retrospective

(To be filled at completion.)

## Context and orientation

### Current state

The `pagination` crate lives at `backend/crates/pagination` and provides the
shared pagination primitives for the Wildside backend. It was introduced by
roadmap item 4.1.1 and extended by 4.1.2 (direction-aware cursors). The crate
is intentionally transport- and persistence-neutral: it knows nothing about
Actix, Diesel, or endpoint-specific schemas.

The crate's source files are:

- `backend/crates/pagination/src/lib.rs` — crate root with re-exports and a
  brief `//!` doc block (currently 55 lines).
- `backend/crates/pagination/src/cursor.rs` — `Cursor<Key>`, `CursorError`,
  `Direction`, encoding/decoding logic, and 11 unit tests (currently 364
  lines).
- `backend/crates/pagination/src/envelope.rs` — `Paginated<T>`,
  `PaginationLinks`, link building, and 1 unit test (currently 204 lines).
- `backend/crates/pagination/src/params.rs` — `PageParams`,
  `PageParamsError`, normalization logic, and 4 unit tests (currently 137
  lines).

The crate's test files are:

- `backend/crates/pagination/tests/pagination_bdd.rs` — BDD test driver
  using `rstest-bdd` with a `World` state machine.
- `backend/crates/pagination/tests/features/pagination.feature` — 8
  Gherkin scenarios covering core pagination behaviour.
- `backend/crates/pagination/tests/features/direction_aware_cursors.feature`
  — 3 Gherkin scenarios covering direction-aware cursors.

The crate's `Cargo.toml` declares these production dependencies: `base64`
0.22, `serde` 1 (with `derive`), `serde_json` 1, `thiserror` 2, `url` 2.
Dev dependencies: `rstest` 0.26, `rstest-bdd` 0.5.0, `rstest-bdd-macros`
0.5.0 (with `strict-compile-time-validation`).

### Public API surface

The crate exports the following items:

- `Cursor<Key>` — generic cursor wrapping an ordering key and a `Direction`.
  Constructors: `new(key)` (default `Next`), `with_direction(key, dir)`.
  Methods: `key()`, `direction()`, `into_inner()`, `into_parts()`,
  `encode()`, `decode(value)`.
- `CursorError` — three variants: `Serialize`, `InvalidBase64`,
  `Deserialize`, each with a `message: String` field.
- `Direction` — `Next` (default) or `Prev`.
- `Paginated<T>` — envelope with `data: Vec<T>`, `limit: usize`,
  `links: PaginationLinks`.
- `PaginationLinks` — `self_: String`, `next: Option<String>`,
  `prev: Option<String>`. Constructors: `new(...)`,
  `from_request(url, params, next_cursor, prev_cursor)`.
- `PageParams` — normalised pagination parameters. Constructor:
  `new(cursor, limit)`. Methods: `cursor()`, `limit()`. Implements
  `Deserialize` with automatic normalization.
- `PageParamsError` — single variant: `InvalidLimit`.
- `DEFAULT_LIMIT: usize` — 20.
- `MAX_LIMIT: usize` — 100.

### Key reference documents

- `docs/keyset-pagination-design.md` — detailed crate design covering cursor
  semantics, ordering requirements, Diesel integration patterns, and OpenAPI
  schema.
- `docs/wildside-backend-architecture.md` — hexagonal architecture reference,
  including the pagination foundation entry (line 1971+) and the pagination
  compatibility requirements (line 2003+).
- `docs/backend-roadmap.md` — roadmap with item 4.1.3 at line 226.
- `docs/documentation-style-guide.md` — spelling, formatting, and Rustdoc
  conventions.
- `docs/rust-doctest-dry-guide.md` — guidance on effective doctests.
- `docs/rust-testing-with-rstest-fixtures.md` — `rstest` fixture patterns.
- `docs/rstest-bdd-users-guide.md` — BDD test conventions with `rstest-bdd`.
- `AGENTS.md` — repository-wide coding and testing standards.

## Plan of work

### Stage A: Audit and preparation (no code changes)

Read the current crate source in full. Catalogue which invariants are already
documented, which are tested, and which are missing. Record the line counts of
each source file to ensure the 400-line limit is respected.

Specifically, confirm:

- The existing `lib.rs` example already demonstrates `PageParams`, `Cursor`,
  and `Paginated` usage but does not explain ordering requirements or error
  mapping.
- The module-level `//!` comments in `cursor.rs`, `envelope.rs`, and
  `params.rs` are each a single line and could benefit from a brief paragraph
  summarising the module's contract and integration guidance.
- Existing unit tests already cover default limit, maximum limit, zero limit
  rejection, cursor round-tripping, and error variants. New tests should
  focus on invariants that are *documented* but not yet *tested* — for
  example, verifying that the documented error display strings are stable, or
  that the documented limit constants are consistent with the normalisation
  logic.

Validation: no code changes; this stage produces only notes in the `Progress`
section.

### Stage B: Expand crate-level documentation in `lib.rs`

Add four new documentation sections to the `//!` block in `lib.rs`, after the
existing `# Example` section:

1. `# Ordering requirements` — explain that key types must implement
   `Serialize` and `DeserializeOwned`, that the key fields must correspond to
   a composite database index providing a total ordering, that ties must be
   broken by a unique field (typically a UUID), and that the ordering must
   remain consistent across all pages of a given endpoint.

2. `# Default and maximum limits` — document `DEFAULT_LIMIT` (20) and
   `MAX_LIMIT` (100), explain that `PageParams` applies the default when no
   limit is provided, caps oversized requests at the maximum, and rejects
   zero as invalid. Reference the constants by name so the documentation
   stays in sync with the code.

3. `# Error mapping guidelines` — describe how consumers should translate
   `CursorError` and `PageParamsError` to HTTP responses. Recommend HTTP 400
   (Bad Request) for all cursor and parameter errors. Provide a short table
   or list mapping each error variant to a suggested response code and
   envelope `code` field value.

4. `# Scope boundaries` — state what the crate intentionally does not
   provide: no Diesel query filters, no Actix extractors, no connection
   pooling, no OpenAPI schema generation. Explain that these responsibilities
   belong to the inbound or outbound adapters that consume the crate.

Keep each section to 15–25 lines of doc comment to stay within the 400-line
file limit. Use short, focused code snippets where they clarify the text.

Validation: `cargo doc -p pagination --no-deps` succeeds without warnings.
`lib.rs` remains under 400 lines.

### Stage C: Improve module-level documentation

Expand the `//!` comments in `cursor.rs`, `envelope.rs`, and `params.rs` from
single-line summaries to short paragraphs (3–6 lines each) that explain the
module's responsibility, its relationship to the rest of the crate, and any
integration guidance specific to that module.

For `cursor.rs`, add a brief note about the base64url JSON encoding format,
the backward-compatibility behaviour of the `dir` field, and the security
consideration that cursors are opaque but not signed.

For `envelope.rs`, add a note about the `from_request` constructor's query
parameter preservation behaviour and the `skip_serializing_if` annotation on
optional links.

For `params.rs`, add a note about the `Deserialize` implementation's
automatic normalization and how it interacts with framework query extractors.

Validation: `cargo doc -p pagination --no-deps` succeeds without warnings. No
source file exceeds 400 lines.

### Stage D: Add BDD scenarios for documentation invariants

Create a new Gherkin feature file at
`backend/crates/pagination/tests/features/pagination_documentation.feature`
covering the documented invariants that are most important for consumer
confidence:

- Default limit is applied when no limit is provided.
- Maximum limit caps oversized requests.
- Zero limit is rejected with an appropriate error.
- Cursor encoding errors produce distinguishable error variants.
- Invalid base64 tokens produce `CursorError::InvalidBase64`.
- Structurally invalid JSON tokens produce `CursorError::Deserialize`.
- Error display strings are human-readable and stable.

Add step definitions to the existing
`backend/crates/pagination/tests/pagination_bdd.rs` file, extending the
`World` state machine where needed.

Validation: `cargo test -p pagination --test pagination_bdd` passes with the
new scenarios.

### Stage E: Add unit tests for documentation-specific invariants

Add new `rstest` unit tests in `params.rs` and `cursor.rs` that verify
invariants newly documented but not yet tested:

- In `params.rs`: verify that `DEFAULT_LIMIT` and `MAX_LIMIT` are consistent
  with the normalization logic (the existing tests cover the behaviour, but a
  test asserting the constant values themselves guards against accidental
  changes to the documented values).
- In `cursor.rs`: verify that `CursorError` display strings contain expected
  substrings (since the documentation recommends specific error codes, the
  display text should remain stable).

Validation: `cargo test -p pagination` passes with the new tests.

### Stage F: Quality gates and documentation updates

Run the full quality gate sequence:

1. `make check-fmt` — formatting compliance.
2. `make lint` — clippy and all linters.
3. `make test` — full test suite.

If all gates pass:

1. Record the documentation design decision in
   `docs/wildside-backend-architecture.md` in the pagination foundation
   section (near line 1971), noting that crate-level documentation now covers
   ordering requirements, default and maximum limits, error mapping
   guidelines, and scope boundaries.
2. Mark roadmap item 4.1.3 done in `docs/backend-roadmap.md` by changing
   `- [ ] 4.1.3.` to `- [x] 4.1.3.`.
3. Update this ExecPlan's status to COMPLETE.

Validation: `make check-fmt`, `make lint`, and `make test` all pass with logs
retained via `tee`.

## Concrete steps

Execute all commands from the repository root (`/home/user/project`).

### 1. Verify current state

```bash
cargo test -p pagination 2>&1 | tee /tmp/4-1-3-baseline-test.log
echo "Exit code: $?"
```

Expected: all existing tests pass (approximately 25+ tests, 0 failures).

### 2. Count current line lengths

```bash
wc -l backend/crates/pagination/src/*.rs
```

Expected: `lib.rs` ~55, `cursor.rs` ~364, `envelope.rs` ~204, `params.rs`
~137. All under 400.

### 3. Expand crate-level documentation

Edit `backend/crates/pagination/src/lib.rs` to add the four new sections
after the existing `# Example` block. Aim for `lib.rs` to be approximately
130–160 lines after the expansion.

### 4. Improve module-level documentation

Edit the `//!` blocks in `cursor.rs`, `envelope.rs`, and `params.rs`. Each
expansion should add 3–8 lines.

### 5. Verify documentation builds

```bash
cargo doc -p pagination --no-deps 2>&1 | tee /tmp/4-1-3-doc-build.log
echo "Exit code: $?"
```

Expected: exit code 0, no warnings.

### 6. Add BDD feature file

Create
`backend/crates/pagination/tests/features/pagination_documentation.feature`
with scenarios covering the documented invariants.

### 7. Add step definitions

Extend `backend/crates/pagination/tests/pagination_bdd.rs` with step
definitions for the new scenarios.

### 8. Add unit tests

Add new tests in `params.rs` and `cursor.rs` for documentation-specific
invariants.

### 9. Run crate-scoped tests

```bash
cargo test -p pagination 2>&1 | tee /tmp/4-1-3-crate-test.log
echo "Exit code: $?"
```

Expected: all tests pass, including new ones.

### 10. Run crate-scoped lint

```bash
cargo clippy -p pagination --all-targets --all-features -- -D warnings 2>&1 \
  | tee /tmp/4-1-3-crate-lint.log
echo "Exit code: $?"
```

Expected: exit code 0, no warnings.

### 11. Run full quality gates

```bash
set -o pipefail
make check-fmt 2>&1 | tee /tmp/4-1-3-check-fmt.log
echo "check-fmt exit: $?"
make lint 2>&1 | tee /tmp/4-1-3-lint.log
echo "lint exit: $?"
make test 2>&1 | tee /tmp/4-1-3-test.log
echo "test exit: $?"
```

Expected: all three exit with code 0.

### 12. Update architecture document

Edit `docs/wildside-backend-architecture.md` to add a design decision entry
in the pagination foundation section.

### 13. Update roadmap

Edit `docs/backend-roadmap.md` to mark item 4.1.3 as done.

### 14. Final verification

```bash
cargo test -p pagination 2>&1 | tee /tmp/4-1-3-final-test.log
echo "Exit code: $?"
```

Expected: all tests pass.

## Validation and acceptance

### Quality criteria (what "done" means)

- Tests:
  - All existing pagination crate tests continue to pass.
  - New BDD scenarios in `pagination_documentation.feature` pass.
  - New unit tests verifying documented invariants pass.
  - `cargo test -p pagination` returns zero failures.

- Lint/typecheck:
  - `cargo clippy -p pagination --all-targets --all-features -- -D warnings`
    passes.
  - `make lint` passes.

- Formatting:
  - `make check-fmt` passes.

- Documentation:
  - `cargo doc -p pagination --no-deps` builds without warnings.
  - The generated HTML contains sections: "Ordering requirements", "Default
    and maximum limits", "Error mapping guidelines", "Scope boundaries".
  - Crate-level documentation uses en-GB-oxendict spelling.
  - Module-level `//!` comments in all four source files are expanded beyond
    single-line summaries.

- Architecture:
  - `docs/wildside-backend-architecture.md` contains a design decision entry
    for roadmap 4.1.3.
  - `docs/backend-roadmap.md` marks item 4.1.3 with `[x]`.

### Quality method (how to check)

```bash
# Crate tests
cargo test -p pagination

# Crate lint
cargo clippy -p pagination --all-targets --all-features -- -D warnings

# Full gates
make check-fmt
make lint
make test

# Documentation build
cargo doc -p pagination --no-deps
```

## Idempotence and recovery

All steps are safe to re-run. Documentation edits and test additions are
additive and do not affect persistent state. If a test fails, fix the issue
and re-run; partial documentation edits do not leave the crate in a broken
state because they are comments rather than code. The embedded Postgres tests
(if any run as part of `make test`) use temporary data directories that are
cleaned up automatically.

## Artifacts and notes

### Expected documentation structure in `lib.rs`

After stage B, the `//!` block in `lib.rs` should follow this outline:

```plaintext
//! Shared opaque cursor and pagination envelope primitives.
//!
//! <existing overview paragraph>
//!
//! # Cursor-based pagination
//! <existing brief heading>
//!
//! # Example
//! <existing code example>
//!
//! # Ordering requirements
//! <new: 15-25 lines on total ordering, composite index, key type bounds>
//!
//! # Default and maximum limits
//! <new: 15-20 lines on DEFAULT_LIMIT, MAX_LIMIT, normalization behaviour>
//!
//! # Error mapping guidelines
//! <new: 15-25 lines mapping CursorError/PageParamsError to HTTP status>
//!
//! # Scope boundaries
//! <new: 10-15 lines on what the crate does not provide>
```

### Expected BDD scenario titles

```plaintext
Feature: Pagination documentation invariants

  Scenario: Default limit is applied when no limit is provided
  Scenario: Maximum limit caps oversized requests
  Scenario: Zero limit is rejected with an error
  Scenario: Invalid base64 token produces InvalidBase64 error
  Scenario: Structurally invalid JSON produces Deserialize error
  Scenario: Error display strings are human-readable
```

### Error mapping reference table

This table summarises the error mapping guidelines that will be documented:

| Error type       | Variant          | Suggested HTTP status | Envelope `code`         |
|------------------|------------------|-----------------------|-------------------------|
| `CursorError`    | `InvalidBase64`  | 400 Bad Request       | `invalid_cursor`        |
| `CursorError`    | `Deserialize`    | 400 Bad Request       | `invalid_cursor`        |
| `CursorError`    | `Serialize`      | 500 Internal Error    | `internal_error`        |
| `PageParamsError`| `InvalidLimit`   | 400 Bad Request       | `invalid_page_params`   |

Note: `CursorError::Serialize` maps to 500 because it indicates a bug in the
server (the key type could not be serialized), not a client error.

## Interfaces and dependencies

No new types, traits, or function signatures are introduced. The existing
public API surface is unchanged. The only additions are:

- Documentation comments (`//!` and `///`).
- Unit tests in `#[cfg(test)]` modules.
- BDD feature files and step definitions.

The crate's dependency set remains unchanged:

- Production: `base64` 0.22, `serde` 1, `serde_json` 1, `thiserror` 2,
  `url` 2.
- Dev: `rstest` 0.26, `rstest-bdd` 0.5.0, `rstest-bdd-macros` 0.5.0.
