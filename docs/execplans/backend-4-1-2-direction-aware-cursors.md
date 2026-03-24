# ExecPlan: Add Direction-Aware Cursors to Pagination Crate

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

## Purpose / big picture

Extend the `pagination` crate at `backend/crates/pagination` to support
direction-aware cursors (`Next` and `Prev`) for bidirectional pagination. This
enables clients to navigate both forward and backward through paginated result
sets using opaque cursors, as specified in `docs/keyset-pagination-design.md`.

After this change, the pagination crate will provide:

- A `Direction` enum (`Next`, `Prev`) embedded in cursors to indicate traversal
direction.
- Updated `Cursor<Key, Direction>` encoding/decoding that preserves direction
through base64url JSON serialization.
- Property tests ensuring encode-decode round-trip stability.
- Unit and behavioural tests covering happy paths, unhappy paths, and edge
cases.

## Constraints

Hard invariants that must hold throughout implementation:

- **Crate boundaries:** The pagination crate must not depend on Actix, Diesel,
or backend domain modules (per hexagonal architecture rules).
- **API compatibility:** Existing cursor encoding/decoding must continue to
work; this change adds direction awareness without breaking existing
functionality.
- **Opaque cursor contract:** Encoded cursors must remain opaque to clients
(base64url JSON) and must not expose internal structure.
- **Test tooling:** All tests must use `rstest` for unit tests and
`rstest-bdd` for behavioural tests, following patterns in
`docs/rust-testing-with-rstest-fixtures.md` and
`docs/rstest-bdd-users-guide.md`.
- **Embedded Postgres:** Integration-style tests must use
`pg-embedded-setup-unpriv` for local testing (where persistence is needed).
- **Quality gates:** `make check-fmt`, `make lint`, and `make test` must all
pass before completion.
- **File size:** No single code file may exceed 400 lines (per
`AGENTS.md` guidelines).

## Tolerances (exception triggers)

Thresholds that trigger escalation when breached:

- **Scope:** If implementation requires changes to more than 8 files or 500
lines of code (net), stop and escalate.
- **Dependencies:** If a new external dependency beyond `serde`, `base64`,
`thiserror`, `url`, `rstest`, or `rstest-bdd` is required, stop and escalate.
- **Interface changes:** If public API signatures must change in a breaking
way, stop and escalate.
- **Iterations:** If tests still fail after 3 attempts, stop and escalate.
- **Time:** If any milestone takes more than 4 hours, stop and escalate.

## Risks

Known uncertainties that might affect the plan:

- **Risk:** Property test discovery of edge cases in cursor encoding/decoding
that require significant refactoring.
  - Severity: low
  - Likelihood: medium
  - Mitigation: Start with simple property tests (round-trip stability), expand
    gradually. The design doc already specifies the cursor format.

- **Risk:** Backward compatibility concerns with existing cursor format.
  - Severity: medium
  - Likelihood: low
  - Mitigation: This is a new feature (4.1.2), not a breaking change. Existing
    cursor functionality (without direction) remains valid; direction-aware
    cursors are additive.

- **Risk:** Integration with pg-embedded-setup-unpriv may require async test
  patterns that complicate the test structure.
  - Severity: low
  - Likelihood: medium
  - Mitigation: Property tests for cursor encoding are pure unit tests and do
    not need Postgres. BDD tests can use the existing sync patterns from
    `pagination/tests/pagination_bdd.rs`.

## Progress

- [ ] (YYYY-MM-DD HH:MMZ) Create `Direction` enum with serde support
- [ ] (YYYY-MM-DD HH:MMZ) Update `Cursor` struct to include direction field
- [ ] (YYYY-MM-DD HH:MMZ) Implement direction-aware encode/decode
- [ ] (YYYY-MM-DD HH:MMZ) Add unit tests with `rstest` for cursor round-trips
- [ ] (YYYY-MM-DD HH:MMZ) Add property tests for encode-decode stability
- [ ] (YYYY-MM-DD HH:MMZ) Add behavioural tests with `rstest-bdd`
- [ ] (YYYY-MM-DD HH:MMZ) Update crate documentation
- [ ] (YYYY-MM-DD HH:MMZ) Run quality gates (`make check-fmt`, `make lint`,
  `make test`)
- [ ] (YYYY-MM-DD HH:MMZ) Update roadmap entry 4.1.2 to "done"
- [ ] (YYYY-MM-DD HH:MMZ) Record design decisions in architecture document

## Surprises & discoveries

*To be filled during implementation.*

## Decision log

*To be filled during implementation.*

## Outcomes & retrospective

*To be filled at completion.*

## Context and orientation

### Current state

The pagination crate at `backend/crates/pagination` provides:

- `Cursor<Key>`: A generic cursor wrapper for ordering keys, with base64url
  JSON encoding/decoding.
- `PageParams`: Query parameter parsing with default limit (20) and max limit
  (100).
- `Paginated<T>`: Response envelope with data, limit, and hypermedia links.
- `PaginationLinks`: Self, next, and prev link generation.

The current `Cursor` only wraps a key value. It does not encode pagination
direction (forward/backward), which limits the ability to generate accurate
prev/next links when traversing result sets.

### Target design

Per `docs/keyset-pagination-design.md`, direction-aware cursors embed a
`Direction` enum:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Next,
    Prev,
}
```

The cursor JSON structure becomes:

```json
{"dir":"Next","key":{"created_at":"2025-10-10T19:17:56Z","id":"...uuid..."}}
```

When decoded, the direction indicates:

- `Next`: The cursor represents the last item of the previous page; fetch items
  *after* this key in sort order.
- `Prev`: The cursor represents the first item of the next page; fetch items
  *before* this key in sort order.

### Key files

- `backend/crates/pagination/src/lib.rs` – Crate root, public exports.
- `backend/crates/pagination/src/cursor.rs` – Cursor encoding/decoding logic.
- `backend/crates/pagination/src/envelope.rs` – PaginationLinks and Paginated.
- `backend/crates/pagination/src/params.rs` – PageParams.
- `backend/crates/pagination/tests/pagination_bdd.rs` – Existing BDD tests.
- `backend/crates/pagination/Cargo.toml` – Crate dependencies.
- `docs/backend-roadmap.md` – Roadmap entry 4.1.2 to mark complete.
- `docs/wildside-backend-architecture.md` – Architecture decision log.

## Plan of work

### Stage A: Scaffolding and direction enum

1. Add `Direction` enum to a new file or extend `cursor.rs`:
   - Derive `Clone`, `Copy`, `Debug`, `PartialEq`, `Eq`, `Serialize`,
     `Deserialize`.
   - Include doc comments explaining `Next` and `Prev` semantics.

2. Update `Cursor<Key>` to `Cursor<Key, Direction = ()>` or add direction as a
   field:
   - Option A (preferred): Add `direction: Direction` field with default
     backward-compatible behavior.
   - Option B: Use generic `Cursor<Key, Dir = ()>` to maintain backward compat.

   Decision: Use Option A with a const default for backward compatibility.

### Stage B: Implementation

3. Modify `Cursor` struct in `cursor.rs`:

   ```rust
   #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
   pub struct Cursor<Key, Direction = ()> {
       key: Key,
       #[serde(skip_serializing_if = "is_unit", default)]
       dir: Direction,
   }
   ```

   Or simpler: always include direction field, default to `Direction::Next` for
   backward compatibility during transition.

4. Add constructor methods:
   - `Cursor::new(key)` – creates cursor with default direction (Next).
   - `Cursor::with_direction(key, direction)` – creates cursor with explicit
     direction.

5. Add accessor methods:
   - `Cursor::direction(&self) -> &Direction`
   - `Cursor::key(&self) -> &Key` (already exists, may need update)
   - `Cursor::into_parts(self) -> (Key, Direction)`

6. Update encoding/decoding:
   - The existing `encode()` and `decode()` should work via serde if Direction
     implements Serialize/Deserialize.
   - Ensure base64url encoding remains unchanged.

### Stage C: Unit tests with `rstest`

7. Add unit tests in `cursor.rs` (within `#[cfg(test)]` module):

   - `direction_round_trips_through_opaque_token`: Encode cursor with Next,
     decode, assert direction preserved.
   - `prev_direction_round_trips`: Same for Prev.
   - `cursor_without_direction_defaults_to_next`: Backward compatibility test.
   - `invalid_direction_json_fails_gracefully`: Error handling for malformed
     direction values.

8. Add property tests using `rstest` with `#[case]` or value combinations:

   - Test all combinations of:
     - Direction: Next, Prev
     - Key types: simple (String), complex (struct with multiple fields)
     - Edge cases: empty strings, special characters in keys

### Stage D: Behavioural tests with `rstest-bdd`

9. Extend or add Gherkin feature file (e.g.,
   `tests/features/direction_aware_cursors.feature`):

   ```gherkin
   Feature: Direction-aware cursors

     Scenario: Encode and decode Next cursor
       Given a composite ordering key
       And direction Next
       When the key and direction are encoded into a cursor and decoded
       Then the decoded cursor has direction Next
       And the decoded cursor key matches the original key

     Scenario: Encode and decode Prev cursor
       Given a composite ordering key
       And direction Prev
       When the key and direction are encoded into a cursor and decoded
       Then the decoded cursor has direction Prev
       And the decoded cursor key matches the original key

     Scenario: Cursor without explicit direction defaults to Next
       Given a composite ordering key
       When the key is encoded into an opaque cursor and decoded
       Then the decoded cursor has direction Next
   ```

10. Add step definitions in a new test file or extend
    `tests/pagination_bdd.rs`:
    - `#[given("direction {direction}")]` – sets direction in world.
    - `#[when("the key and direction are encoded into a cursor and decoded")]`
      – performs round-trip.
    - `#[then("the decoded cursor has direction {expected}")]` – asserts
      direction.

### Stage E: Documentation and quality gates

11. Update crate-level documentation in `lib.rs`:
    - Add example showing direction-aware cursor usage.
    - Document the Direction enum semantics.

12. Run quality gates:
    - `make check-fmt` – ensure formatting passes.
    - `make lint` – ensure clippy and other lints pass.
    - `make test` – ensure all tests pass.

13. Update roadmap:
    - Mark `docs/backend-roadmap.md` item 4.1.2 as done.

14. Record design decisions:
    - Add entry to `docs/wildside-backend-architecture.md` decision log
      documenting the direction-aware cursor design.

## Concrete steps

Execute these commands from the repository root (`/data/leynos/Projects/wildside.worktrees/backend-4-1-2-direction-aware-cursors`).

### 1. Verify current state

```bash
cargo test -p pagination
```

Expected: All existing tests pass.

### 2. Implement Direction enum and update Cursor

Edit `backend/crates/pagination/src/cursor.rs`:

- Add `Direction` enum after imports.
- Update `Cursor` struct to include `dir: Direction` field.
- Update constructors and methods.

### 3. Add unit tests

Add tests to the `#[cfg(test)]` module in `cursor.rs`:

```rust
#[rstest]
#[case(Direction::Next)]
#[case(Direction::Prev)]
fn direction_round_trips_through_encoding(#[case] direction: Direction) {
    let cursor = Cursor::with_direction(
        FixtureKey { created_at: "2026-03-22T10:30:00Z".to_owned(), id: "test".to_owned() },
        direction,
    );
    let encoded = cursor.encode().expect("encoding succeeds");
    let decoded = Cursor::<FixtureKey, Direction>::decode(&encoded).expect("decoding succeeds");
    assert_eq!(decoded.direction(), &direction);
}
```

### 4. Run unit tests

```bash
cargo test -p pagination cursor
```

Expected: New direction tests pass.

### 5. Add behavioural tests

Create `backend/crates/pagination/tests/features/direction_aware_cursors.feature`:

```gherkin
Feature: Direction-aware cursor pagination

  Scenario: Next direction round-trips through encoding
    Given a composite ordering key
    And pagination direction Next
    When the key and direction are encoded into a cursor and decoded
    Then the decoded cursor has direction Next
    And the decoded cursor key matches the original key

  Scenario: Prev direction round-trips through encoding
    Given a composite ordering key
    And pagination direction Prev
    When the key and direction are encoded into a cursor and decoded
    Then the decoded cursor has direction Prev
    And the decoded cursor key matches the original key
```

Add step definitions to `tests/pagination_bdd.rs` or create
`tests/direction_aware_bdd.rs`.

### 6. Run behavioural tests

```bash
cargo test -p pagination --test pagination_bdd
```

(or the new test file if created separately)

Expected: All BDD scenarios pass.

### 7. Run quality gates

```bash
make check-fmt
make lint
make test
```

Expected: All pass.

### 8. Update documentation

Edit `backend/crates/pagination/src/lib.rs`:

- Update module docstring to mention Direction support.
- Add example showing `Cursor::with_direction()`.

Edit `docs/wildside-backend-architecture.md`:

- Add design decision entry for direction-aware cursors in section
  "Design decisions".

### 9. Update roadmap

Edit `docs/backend-roadmap.md`:

- Change `- [ ] 4.1.2.` to `- [x] 4.1.2.`.

## Validation and acceptance

### Quality criteria (what "done" means)

- **Tests:**
  - Unit tests in `cursor.rs` cover:
    - Round-trip encoding/decoding with Next direction
    - Round-trip encoding/decoding with Prev direction
    - Error handling for invalid direction values in JSON
    - Backward compatibility (cursors without direction default appropriately)
  - Behavioural tests in BDD feature file cover:
    - Happy path: Next direction round-trip
    - Happy path: Prev direction round-trip
    - Edge case: Mixed key types with direction
  - All tests pass: `cargo test -p pagination` returns 0 failures.

- **Lint/typecheck:**
  - `make lint` returns no errors or warnings.
  - `cargo clippy -p pagination -- -D warnings` passes.

- **Formatting:**
  - `make check-fmt` passes (or `cargo fmt -- --check` shows no changes
    needed).

- **Documentation:**
  - Crate-level docs in `lib.rs` include Direction usage example.
  - Architecture decision logged in `docs/wildside-backend-architecture.md`.

### Quality method (how we check)

```bash
# Run all pagination crate tests
cargo test -p pagination

# Run formatting check
cargo fmt -- --check

# Run lints
cargo clippy -p pagination -- -D warnings

# Run full quality gates
make check-fmt
make lint
make test
```

## Idempotence and recovery

- Running `cargo test` multiple times is safe and produces the same results.
- If a test fails, fix the issue and re-run; partial implementations do not
  leave persistent state.
- The embedded Postgres tests (if any) use temporary data directories that are
  cleaned up automatically.

## Artifacts and notes

### Expected test output

```plaintext
$ cargo test -p pagination

running 15 tests
tests::cursor_round_trips_through_opaque_token ... ok
tests::invalid_base64_cursor_fails_decode ... ok
tests::padded_base64_cursor_decodes_successfully ... ok
tests::structurally_invalid_json_cursor_fails_decode ... ok
tests::direction_round_trips_through_encoding_next ... ok
tests::direction_round_trips_through_encoding_prev ... ok
tests::cursor_with_direction_defaults_to_next ... ok
...
test result: ok. 15 passed; 0 failed; 0 ignored
```

### Interface definitions

At completion, the following types and functions must exist:

In `backend/crates/pagination/src/cursor.rs`:

```rust
/// Direction of pagination relative to the cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    /// Forward in the sort order (e.g., newer items if sorting ascending).
    Next,
    /// Backward in the sort order (e.g., older items).
    Prev,
}

/// Cursor wrapper for an ordered boundary key with optional direction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cursor<Key, Dir = ()> {
    key: Key,
    #[serde(skip_serializing_if = "is_unit", default)]
    dir: Dir,
}

impl<Key> Cursor<Key, ()> {
    pub const fn new(key: Key) -> Self;
}

impl<Key, Dir> Cursor<Key, Dir> {
    pub fn with_direction(key: Key, dir: Dir) -> Self;
    pub const fn key(&self) -> &Key;
    pub const fn direction(&self) -> &Dir;
    pub fn into_parts(self) -> (Key, Dir);
}
```

In `backend/crates/pagination/src/lib.rs`:

```rust
pub use cursor::{Cursor, CursorError, Direction};
```

## Revision note

*Initial version created. Awaiting approval before implementation.*
