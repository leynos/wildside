# Add property-based tests for route cache key canonicalization invariants

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, `Outcomes & Retrospective`, `Conformance Basis`, and
`Verification Plan` must be kept up to date as work proceeds.

Status: DRAFT

## Purpose / big picture

Roadmap item 5.1.4 shipped canonical route cache key derivation
(`RouteCacheKey::for_route_request` in `backend/src/domain/ports/cache_key.rs`)
with example-based contract tests. The test module still carries this marker:

```rust
//! TODO: Add property-based tests for canonicalization invariants across
//! generated key ordering, theme arrays, and coordinate rounding cases.
```

This follow-up, tracked as roadmap item 5.1.4a, discharges that TODO. It adds
property-based tests (using the `proptest` crate, which generates many random
inputs and shrinks failures to minimal counter-examples) that state the
canonicalization invariants over generated route-request payloads rather than
hand-picked examples: theme-array permutation invariance, coordinate rounding
equivalence and divergence, normalization idempotence, negative-zero
collapse, order preservation for non-canonicalized arrays, and the
`route:v1:<64-character lowercase hexadecimal SHA-256>` key format.

There is no user-visible behaviour change. The observable outcome for a
developer is: `cargo test -p backend cache_key` runs the new property suite,
each property demonstrably fails when the canonicalization logic is broken
(seeded-fault evidence is recorded below), and the TODO comment is gone.

## Context and orientation

Wildside is a Rust backend organized hexagonally: domain code under
`backend/src/domain/` defines ports; adapters under `backend/src/outbound/`
implement them. Route plans are cached in Redis under canonical keys so that
semantically equivalent requests share one cache entry.

Key locations:

- `backend/src/domain/ports/cache_key.rs` (377 lines) — the canonicalization
  seam. `RouteCacheKey::for_route_request(payload: &serde_json::Value)`
  normalizes the payload via the private `normalize_route_request_value`,
  hashes it through `crate::domain::idempotency::PayloadHash` (SHA-256 over
  compact canonical JSON), and formats `route:v1:<hex digest>`. The inline
  `#[cfg(test)] mod tests` (lines 192–377) holds `rstest` example cases and
  `insta` snapshot tests.
- Normalization semantics (from the current implementation):
  - Object entries are collected and explicitly sorted by key at every depth.
  - Arrays whose immediate containing object key is one of `SORTED_ARRAY_KEYS`
    (`themes`, `themeIds`, `interestThemeIds`) are sorted lexicographically,
    but only when every element is a JSON string; any non-string element
    leaves the whole array in original order. Duplicates are preserved.
  - Array elements recurse with `current_key = None`, so eligibility for
    sorting or rounding never propagates through an array boundary; however,
    objects inside arrays regain per-field eligibility (a `lat` field inside
    `waypoints: [{...}]` is still rounded).
  - Numbers whose immediate containing key is one of `ROUNDED_COORDINATE_KEYS`
    (`lat`, `lng`, `lon`, `latitude`, `longitude`) are rounded to five decimal
    places via `(value * 100_000.0).round() / 100_000.0`; a result equal to
    zero is canonicalized to positive zero. `Number::from_f64` failure falls
    back to the original number.
  - Values under coordinate keys that are not numbers (strings, objects,
    arrays, booleans, null) pass through untouched.
- `backend/src/domain/idempotency/payload.rs` — `canonicalize_and_hash`
  provides the SHA-256 digest over key-sorted, compactly serialized JSON.
- `backend/tests/route_cache_key_canonicalization_bdd.rs` with
  `backend/tests/features/route_cache_key_canonicalization.feature` — one
  Redis-backed `rstest-bdd` scenario proving two fixed, semantically
  equivalent payloads share a cache slot. It remains unchanged.
- `backend/src/outbound/queue/apalis_route_queue.rs` declares
  `#[cfg(test)] mod tests;` resolving to `apalis_route_queue/tests.rs`, which
  in turn declares `mod properties;` (`tests/properties.rs`). This is the
  repository's precedent for splitting a large test module and for housing
  property tests in their own file.

Facts established during planning that shape this design:

- `serde_json` is compiled without the `preserve_order` feature (confirmed
  from `Cargo.lock`: no `indexmap` dependency on `serde_json` 1.0.150), so
  `serde_json::Value::Object` is backed by `BTreeMap` and object key order is
  already sorted at the `Value` representation level. A property asserting
  "object key insertion order does not affect the key" is therefore vacuous:
  it cannot fail even if the explicit sort in
  `normalize_route_request_value` were deleted. See `Decision Log`.
- `serde_json::Number::as_f64` in this build always returns `Some` (verified
  against the pinned 1.0.150 sources: `PosInt`/`NegInt` are lossily cast with
  `as f64`), so the `None` fallback in `round_coordinate` is unreachable, and
  integer coordinates beyond 2^53 lose precision silently.
- Non-finite floats (NaN, infinities) cannot be constructed through the
  public `serde_json::Value` API (`Number::from_f64` rejects them) nor parsed
  from JSON text, so generators need not (and cannot) cover them.
- `proptest = "1"` is already a dev-dependency (`backend/Cargo.toml:92`). No
  new dependencies are required.
- No `proptest-regressions/` directories exist yet and `.gitignore` does not
  exclude them; the proptest failure-persistence convention is to commit them.

Relevant guides to read before implementing: the `proptest` skill,
`docs/rust-testing-with-rstest-fixtures.md`, `docs/rstest-bdd-users-guide.md`
(for why the existing BDD suite is left alone),
`docs/wildside-backend-architecture.md` (route caching contract, lines
1938–1950), `docs/complexity-antipatterns-and-refactoring-strategies.md`
(module-split hygiene), and the `hexagonal-architecture` and
`rust-unit-testing` skills.

## Constraints

- Test-only change: no production code paths in
  `backend/src/domain/ports/cache_key.rs` may change behaviour. The only
  permitted edits to non-test production source are the removal of the TODO
  comment and the conversion of the inline `mod tests { ... }` block into a
  `#[cfg(test)] mod tests;` declaration.
- Keep the canonicalization seam in the domain; do not move logic into
  `backend/src/outbound/cache/redis_route_cache.rs` or test through the Redis
  adapter.
- No new dependencies (dev or production). Use the existing `proptest`,
  `rstest`, `insta`, and `pretty_assertions` dev-dependencies.
- Respect the repository's 400-line file limit (`AGENTS.md`): after the
  split, `cache_key.rs`, `cache_key/tests.rs`, and
  `cache_key/tests/properties.rs` must each stay under 400 lines.
- Existing tests (unit, snapshot, doctest, BDD) must continue to pass
  unmodified apart from the mechanical relocation of the unit-test module.
- Property bodies must use `prop_assert!`/`prop_assert_eq!`/`prop_assert_ne!`
  (not `panic!`/`unwrap`) so shrinking works.
- Strategies must construct valid inputs by composition; `prop_filter` and
  `prop_assume!` are only acceptable for genuinely rare rejections, never for
  structural constraints (the filtering trap).

## Tolerances (exception triggers)

- Scope: more than 6 files touched, or more than 50 net non-test production
  lines, means stop and escalate.
- Interface: if any public API signature must change, or a private function
  must be made `pub` (beyond `pub(crate)`/`super` visibility already granted
  to child test modules), stop and escalate.
- Dependencies: any new crate requirement means stop and escalate.
- Iterations: if a property is still flaky or failing after two focused
  investigate-and-fix loops, stop, record the shrunk counter-example, and
  escalate — a genuine counter-example is a production bug and is out of
  scope for a test-only change.
- Runtime: if the property suite adds more than 30 seconds to
  `cargo test -p backend --lib` on the development machine, reduce case
  counts or generator depth and record the decision.

## Risks

- Risk: a property finds a real counter-example in shipped canonicalization
  behaviour (for example, float rounding near grid boundaries producing
  unequal keys for values the property deems equivalent).
  Severity: medium. Likelihood: medium.
  Mitigation: phrase the coordinate-equivalence property in terms of the
  implementation's contract (equal keys when the rounded values are equal)
  rather than an idealized grid model; if a genuine bug surfaces, stop per
  the iteration tolerance, commit the regression file, and escalate with the
  shrunk input rather than patching production code under a test-only plan.
- Risk: floating-point subtlety makes the divergence property (different
  rounded values imply different keys) flaky if generated pairs straddle a
  boundary the generator did not intend.
  Severity: low. Likelihood: medium.
  Mitigation: derive both coordinates in a pair from one integer grid cell
  (construct-by-composition), so equivalence and divergence are decided by
  integer arithmetic, not by re-deriving the float rounding.
- Risk: the module split breaks doctest or visibility assumptions (the test
  module uses `super::{SORTED_ARRAY_KEYS, ROUNDED_COORDINATE_KEYS, ...}`).
  Severity: low. Likelihood: low.
  Mitigation: child file modules retain identical `super::` visibility; the
  split is mechanical and gate-checked in its own commit.
- Risk: default 256-case runs slow the suite noticeably given SHA-256 hashing
  per case.
  Severity: low. Likelihood: low.
  Mitigation: keep payload generators shallow (recursion depth ≤ 4, ≤ 8 keys
  per object); measure with the runtime tolerance above.

## Conformance basis

- `docs/backend-roadmap.md` §5.1: items 5.1.1–5.1.4 are complete; this plan
  adds and discharges item 5.1.4a (the roadmap entry is created by this
  work — see `Plan of work`). Identifier: RM-5.1.4a.
- `docs/wildside-backend-architecture.md`:
  - lines 1574–1579 (domain model): `RouteCacheKey` owns canonical derivation;
    adapters never canonicalize. Identifier: ARCH-CACHEKEY-OWNERSHIP.
  - lines 1938–1950 (route caching): the canonical form sorts the three theme
    arrays, rounds coordinates to five decimal places, serializes JSON with
    stable key ordering, and hashes with SHA-256; keys follow
    `route:v1:<sha256>`. Identifier: ARCH-CACHEKEY-CONTRACT.
- Prior art (external, informative only): RFC 8785 (JSON Canonicalization
  Scheme) motivates hashing a canonical JSON form; this repository uses its
  own simpler canonical form (BTreeMap key order plus compact
  `serde_json::to_vec`), not RFC 8785 number/string formatting. No
  conformance to RFC 8785 is claimed or planned.
- No Terms of Reference document exists for this work; the roadmap and the
  architecture document are the upstream artefacts.

Trace links:

```plaintext
ARCH-CACHEKEY-CONTRACT -> RM-5.1.4a -> EP-M2 -> cache_key::tests::properties::{theme_permutations_share_a_key, coordinates_in_one_grid_cell_share_a_key, distinct_grid_cells_diverge, normalization_is_idempotent, derived_keys_match_route_v1_format, non_theme_array_order_is_material, non_string_theme_arrays_preserve_order}
ARCH-CACHEKEY-OWNERSHIP -> EP-M1 -> module split keeps tests inside the domain port module (no adapter involvement)
```

## Verification plan

The change introduces no new production invariant; it verifies existing ones.
The obligations below are the invariants the shipped implementation is
claimed to satisfy, now checked over generated input ranges. Methods are
property tests throughout: the input domain (arbitrary JSON payloads,
permutations, float ranges) is far too large to enumerate, no new lemma or
contractual logic is introduced (so a Verus proof would have nothing
non-trivial to prove beyond restating the code), and the properties are not
bounded-state or concurrency problems (so Kani, loom, and state-machine
checking are disproportionate). Kani was considered for
`is_lowercase_hex_digest` exhaustion but the function is a two-line character
class check already pinned by example tests; a harness would restate it.

Axioms (external interfaces treated as correct, not verified here):

- AX-1: `sha2::Sha256` implements FIPS 180-4 SHA-256 (collision behaviour is
  not tested; "different canonical bytes give different digests" is assumed
  for the divergence properties, which is sound for test purposes because a
  collision would be a cryptographic event).
- AX-2: `serde_json` 1.0.150 without `preserve_order`: `Value::Object` is
  `BTreeMap`-backed; `to_vec` is deterministic for a given `Value`;
  `Number::from_f64` rejects non-finite values; `Number::as_f64` always
  returns `Some` in this build.
- AX-3: `proptest` 1.x generates values from the stated strategies and
  shrinks failures; default 256 cases per property.

Obligations. Each is exercised through the public entry point
`RouteCacheKey::for_route_request` unless stated; `normalize` refers to the
private `normalize_route_request_value` reachable from the child test module
via `super::`.

- Obligation V-1 (theme permutation invariance): for every generated payload
  containing an all-string array under each of `themes`, `themeIds`, and
  `interestThemeIds` (covering duplicates and empty arrays), every
  permutation of each such array yields the same cache key.
  Method: property test. Domain: string vectors length 0..8 drawn from a
  small alphabet (to force duplicates), shuffled via `Just(vec)` plus
  `proptest::sample::Index`-driven permutation; payloads place the array both
  at the top level and nested under `preferences`.
  Artefact: `backend/src/domain/ports/cache_key/tests/properties.rs`,
  `theme_permutations_share_a_key`.
  Evidence: `cargo test -p backend --lib cache_key::tests::properties`.
  Non-vacuity: generator statistics include vectors with length ≥ 2 and
  non-identity permutations (witness class); negative control NC-1 (invert
  the sort comparator in `normalize_route_request_value` to `b.cmp(a)`) must
  make this property fail with a shrunk two-element array.
- Obligation V-2 (coordinate rounding equivalence): two payloads identical
  except that a coordinate field carries different `f64` values which round
  to the same five-decimal value yield the same key. Pairs are constructed
  from one integer grid cell: draw `cell: i64` in
  `-9_000_000_000..=9_000_000_000` (covering ±90 000 degrees, comfortably
  beyond real coordinates) and two offsets in `-0.49..=0.49` of one grid
  unit, giving `(cell as f64 + offset) / 100_000.0`. Only pairs whose
  perturbed values still round to `cell` are constructed (checked by integer
  arithmetic on the offset, not by filtering).
  Method: property test over each key in `ROUNDED_COORDINATE_KEYS`, at the
  top level and inside an object inside an array (the `waypoints` shape), to
  pin the recursion behaviour discovered during recon.
  Artefact: same file, `coordinates_in_one_grid_cell_share_a_key`.
  Non-vacuity: the offset class includes nonzero offsets of both signs;
  negative control NC-2 (change `COORDINATE_PRECISION_FACTOR` to `1_000_000.0`)
  must make this property fail.
- Obligation V-3 (coordinate divergence): payloads identical except for
  coordinate values drawn from different grid cells (cells differing by ≥ 1)
  yield different keys.
  Method: property test; the pair shares one generator drawing
  `cell` and a nonzero `delta: i64` in `1..=1_000`, using `cell` and
  `cell + delta`.
  Artefact: same file, `distinct_grid_cells_diverge`.
  Non-vacuity: relies on AX-1 (no SHA-256 collisions); negative control NC-3
  (make `round_coordinate` return `0.0` unconditionally) must fail this
  property while leaving V-2 passing — this pair of controls demonstrates the
  two properties are independent, not restatements.
- Obligation V-4 (normalization idempotence): for every generated payload,
  `normalize(normalize(v)) == normalize(v)`, and consequently
  `for_route_request(v) == for_route_request(&normalize(v))`. This catches
  residual float noise from the multiply-round-divide construction: if a
  second rounding pass changed anything, canonicalization would not be a
  projection and equivalent inputs could disagree.
  Method: property test over a general recursive JSON strategy
  (`prop_recursive`, depth ≤ 4, ≤ 8 collection items, leaves covering null,
  bool, integer, float, string) whose object-key strategy is biased to emit
  the special key names (`themes`, `lat`, `lng`, ...) with high probability
  so the sort and round branches are actually reached.
  Artefact: same file, `normalization_is_idempotent`.
  Non-vacuity: assert via a generator statistic (`prop_assert` on a counter
  is not idiomatic; instead a companion `rstest` case checks the strategy
  emits special keys within 512 samples — see `Plan of work`); negative
  control NC-4 (append a constant to every string during normalization)
  must fail idempotence.
- Obligation V-5 (key format totality): every generated payload derives
  `Ok(key)` whose string matches `^route:v1:[0-9a-f]{64}$`, and
  `RouteCacheKey::new(key.as_str())` re-validates it. Error paths of
  derivation are unreachable for `Value` inputs in this build (AX-2); this
  property documents that totality.
  Method: property test over the same recursive strategy as V-4.
  Artefact: same file, `derived_keys_match_route_v1_format`.
  Non-vacuity: negative control NC-5 (uppercase one hex digit in the
  formatted key before validation) must fail; the format check uses explicit
  character-class assertions rather than a regex crate (no new deps).
- Obligation V-6 (order is material where canonicalization is not claimed):
  permuting (with a non-identity permutation) an array of ≥ 2 distinct
  strings under a key not in `SORTED_ARRAY_KEYS` changes the key; likewise
  permuting a theme-keyed array containing at least one non-string element
  (mixed types) changes the key when the permutation moves elements.
  This pins the all-string guard and the documented scope of sorting: cache
  correctness (no false sharing) depends on unequal requests hashing
  unequally.
  Method: property test; distinctness by construction (indexed strings
  `item-0`, `item-1`, ...), non-identity permutation by rotation of a vector
  of length ≥ 2.
  Artefact: same file, `non_theme_array_order_is_material` and
  `non_string_theme_arrays_preserve_order`.
  Non-vacuity: negative control NC-6 (sort every array unconditionally in
  normalization) must fail both.
- Obligation V-7 (negative zero collapse): any coordinate value that rounds
  to zero (offsets within the zero grid cell, both signs) produces the same
  key as literal `0.0`, i.e. `-0.00000049` and `0.0000004` share a key.
  Method: property test, small-offset generator around zero.
  Artefact: same file, folded into `coordinates_in_one_grid_cell_share_a_key`
  as the `cell == 0` class plus an explicit `rstest` example for `-0.0` (the
  `json!(-0.0)` literal) since proptest floats rarely emit signed zero.
  Non-vacuity: negative control NC-7 (remove the `rounded == 0.0` collapse)
  must fail the `-0.0` example; the property's zero-cell class is reported by
  a companion sample-statistics check.

Deliberately not verified, with rationale:

- Object key-order invariance: vacuous in this build (AX-2, BTreeMap); it
  cannot fail whatever the implementation does. Recording this here and in
  the module documentation instead of writing a tautological property. If
  `preserve_order` is ever enabled, this decision must be revisited — a note
  to that effect goes into the property module's doc comment.
- Large-integer precision loss (`i64` beyond 2^53 under `lat`): current
  behaviour is lossy rounding via `as f64`; real coordinates are bounded by
  ±180, so asserting the lossy behaviour would freeze an accident of the
  implementation rather than a contract. Documented as a known edge in the
  module doc comment; no property pins it.
- SHA-256 digest values themselves: covered by the existing example test and
  the idempotency payload tests; re-deriving expected digests in the property
  suite would re-implement the function under test (an anti-pattern).

Negative controls NC-1..NC-7 are executed once during development (Stage B/C
of `Plan of work`) as temporary, uncommitted mutations; the shrunk failure
output for each is pasted into `Artefacts and notes` before the mutation is
reverted. They are not permanent test fixtures.

## Plan of work

Stage A (no code changes): confirm the recon findings still hold on the
implementation branch (`leta show` on `normalize_route_request_value`,
`round_coordinate`, and the tests module; `cargo tree -p backend -i serde_json`
to re-confirm no `preserve_order`).

Stage B (mechanical split, first plateau): convert the inline test module
into child files following the `apalis_route_queue` precedent.

1. In `backend/src/domain/ports/cache_key.rs`, replace
   `#[cfg(test)] mod tests { ... }` (lines 192–377) with
   `#[cfg(test)] mod tests;` and delete nothing else. The TODO comment moves
   with the module and is deleted only in Stage D when the properties exist.
2. Create `backend/src/domain/ports/cache_key/tests.rs` containing the
   existing example tests verbatim (imports adjusted from `super::` context,
   which is unchanged for a child file module) plus `mod properties;`
   (added in Stage C; in Stage B the declaration is absent so the tree stays
   green).

Stage C (red-then-green properties): add
`backend/src/domain/ports/cache_key/tests/properties.rs`.

1. Strategy helpers first: `theme_array()` (small-alphabet string vectors),
   `grid_cell_pair()` (integer cell plus two in-cell offsets),
   `divergent_cells()` (cell plus nonzero delta), `route_payload()`
   (recursive JSON via `prop_recursive` with key names biased toward the
   special keys), and `rotate<T>(Vec<T>, usize) -> Vec<T>` for non-identity
   permutations. Free functions returning `impl Strategy<Value = T>`, matching
   the `generate_route/tests.rs` house style; no `prop_compose!` (repository
   has no precedent for it).
2. A `rstest` sample-statistics check `route_payload_reaches_special_keys`:
   draw 512 values through `proptest::strategy::ValueTree` on a fixed seed
   and assert at least one generated payload contains a theme key with an
   all-string array of length ≥ 2 and at least one rounded-coordinate key
   with a fractional value. This is the standing non-vacuity guard for V-4
   and V-5 (the seeded-fault controls NC-1..NC-7 are one-off development
   evidence; this check stays in the suite).
3. Properties V-1 through V-7 as named in the `Verification plan`, using
   `proptest! { #[test] ... }` blocks with `prop_assert_eq!`/`prop_assert_ne!`
   and default configuration (no `ProptestConfig` override, matching the
   repository's three existing property suites).
4. Red evidence: because production code is already believed correct, the
   red stage is supplied by the negative controls. For each of NC-1..NC-7,
   apply the mutation, run the focused property, capture the shrunk
   counter-example into `Artefacts and notes`, revert, and re-run to green.
   A property whose negative control does not fail is vacuous and must be
   rewritten before proceeding.

Stage D (documentation and closure):

1. Delete the TODO lines from `cache_key/tests.rs` module docs; extend the
   module doc comment of `properties.rs` with the two documented exclusions
   (key-order vacuity under BTreeMap; large-integer coordinates).
2. `docs/backend-roadmap.md`: insert under §5.1, after 5.1.4:
   `- [ ] 5.1.4a. Add property-based tests for cache key canonicalization`
   `invariants (theme permutation invariance, coordinate rounding`
   `equivalence and divergence, normalization idempotence, key format).`
   It is ticked to `[x]` only when this plan reaches COMPLETE.
3. `docs/wildside-backend-architecture.md` route-caching section (after line
   1950): one sentence noting the canonicalization contract is enforced by
   example-based and property-based tests in
   `backend/src/domain/ports/cache_key/tests/`.
4. `docs/developers-guide.md`: a short subsection under the testing
   conventions recording the property-test conventions this work
   consolidates: property modules live in `tests/properties.rs` child
   modules, strategies are free functions composing valid values (no
   structural filtering), bodies use `prop_assert*`, any
   `proptest-regressions/` files produced by failures are committed, and
   shrunk failures are promoted to named `rstest` cases.
5. `docs/users-guide.md`: no change — there is no user-visible behaviour
   change; record that conclusion in `Decision Log` rather than editing the
   guide.
6. If any regression files were produced during development, commit them and
   promote the shrunk inputs to named `rstest` cases in `tests.rs`.

Test-framework applicability, per the repository brief: `rstest` is used for
the example-shaped checks (sample statistics, `-0.0` literal); `proptest`
carries the invariants; `insta` snapshots and `googletest` assertions are not
applicable here (no new multivariant output formats, and `googletest` is not
a dependency of this crate — see `Decision Log`); no new `rstest-bdd` scenario
is added because no externally observable workflow changes (the existing
Redis BDD scenario already covers the end-to-end canonicalization contract);
`kani`/`verus` are disproportionate for the reasons given in the
`Verification plan`.

## Milestones and plateaus

- EP-M1 (mechanical test-module split). Outcome: `cache_key.rs` shrinks to
  ~195 lines of production code plus `#[cfg(test)] mod tests;`; all existing
  tests pass unchanged. Requirements: none discharged; enables RM-5.1.4a.
  Acceptance evidence: `cargo test -p backend --lib cache_key` passes with
  the same test count as before the split; `make check-fmt` and `make lint`
  clean. Conformance check: no public interface, dependency, or format
  change; ARCH-CACHEKEY-OWNERSHIP untouched. Recovery: single mechanical
  commit; revert restores the inline module. Remaining gaps: properties.
  Compatibility decision: none required (internal test surface).
- EP-M2 (property suite green with non-vacuity evidence). Outcome:
  `properties.rs` in place, V-1..V-7 passing, NC-1..NC-7 transcripts recorded
  in this document, sample-statistics guard passing. Acceptance evidence:
  `cargo test -p backend --lib cache_key::tests::properties` lists the seven
  properties plus the statistics check, all passing; each negative-control
  transcript shows the intended failure. Conformance check:
  ARCH-CACHEKEY-CONTRACT verified over generated domains; no production
  behaviour change (diff of `cache_key.rs` against EP-M1 is empty apart from
  Stage D comment removal, which lands in EP-M3). Recovery: properties are
  additive; reverting the commit restores EP-M1. Remaining gaps: docs.
- EP-M3 (documentation, roadmap, TODO removal). Outcome: TODO gone, roadmap
  item 5.1.4a present and ticked, architecture and developers' guides
  updated, all gates green. Acceptance evidence: `make check-fmt`,
  `make lint`, `make test` all pass (subject to the known pre-existing
  Whitaker version skew on `make lint` — verify failures are only the known
  ones by reading the log, not the exit code); `git grep "TODO: Add
  property-based tests" backend/src/domain/ports` returns nothing.
  Recovery: docs-only commit, trivially revertable.

## Concrete steps

All commands run from the repository root. Long outputs go through `tee` to
`/tmp/$ACTION-wildside-backend-5-1-4a-cache-key-canonicalization-property-tests.out`
per the workspace convention. Gates are run sequentially, never in parallel,
and full-gate runs are delegated to the `scrutineer` subagent where possible.

```bash
# Stage A
cargo tree -p backend -i serde_json | head -20   # expect no indexmap parent

# Stage B
cargo test -p backend --lib cache_key 2>&1 | tee /tmp/test-wildside-b514a-m1.out
# expect: identical pass count to pre-split baseline (currently 12 unit tests)

# Stage C (focused loop)
cargo test -p backend --lib cache_key::tests::properties 2>&1 | tee /tmp/test-wildside-b514a-m2.out
# expect: 7 property tests + 1 statistics test, 0 failures

# Stage D / gates (delegate to scrutineer; sequential)
make check-fmt 2>&1 | tee /tmp/check-fmt-wildside-b514a.out
make lint      2>&1 | tee /tmp/lint-wildside-b514a.out
make test      2>&1 | tee /tmp/test-wildside-b514a.out
```

Note: `make check-fmt` and `make lint` are known to exit 0 even when
sub-checks fail; read the tee'd logs, not the exit codes. `make lint` may
show pre-existing Whitaker suite findings caused by a local 0.2.7 vs CI 0.2.6
version skew; only failures newly introduced by this change block progress.

Commit after each stage (Stage B, Stage C, each Stage D document), using
file-based commit messages per the `commit-message` skill.

## Validation and acceptance

Acceptance is behavioural:

1. Before Stage C, `cargo test -p backend --lib cache_key::tests::properties`
   reports zero tests (module absent). After Stage C it reports the seven
   named properties and the statistics guard, all passing.
2. Red evidence: for each negative control NC-1..NC-7, the recorded
   transcript in `Artefacts and notes` shows the focused property failing
   with a shrunk counter-example while the mutation is applied, and passing
   after revert. Example expected shape for NC-1:

   ```plaintext
   Test failed: assertion failed: `(left == right)` ... minimal failing input:
   themes = ["a", "b"], perm = [1, 0]
   ```

3. `make check-fmt`, `make lint`, and `make test` logs show no new failures.
4. `docs/backend-roadmap.md` contains the ticked 5.1.4a entry; the TODO
   comment is gone from the ports module.

## Idempotence and recovery

Every stage is an additive or mechanical commit; re-running any command is
safe. If a property fails intermittently, the committed
`proptest-regressions/` seed makes the failure reproducible
(`cargo test -p backend --lib cache_key::tests::properties` replays stored
seeds first). Rollback at any plateau is `git revert` of that stage's commit;
no data, schema, or wire format is touched.

## Interfaces and dependencies

No interface changes. The property module consumes, via `super::` and crate
paths:

- `crate::domain::ports::cache_key::{RouteCacheKey, RouteCacheKeyDerivationError, RouteCacheKeyValidationError}`
- `super::super::{normalize_route_request_value, SORTED_ARRAY_KEYS, ROUNDED_COORDINATE_KEYS, COORDINATE_PRECISION_FACTOR}` (private items visible to descendant test modules)
- dev-dependencies already present: `proptest = "1"`, `rstest = "0.26"`,
  `serde_json` (`json!` macro), `pretty_assertions` where a non-property
  equality assertion reads better.

## Progress

- [ ] Stage A: re-confirm recon facts on the implementation branch.
- [ ] EP-M1 / Stage B: split `cache_key.rs` tests into `cache_key/tests.rs`.
- [ ] EP-M2 / Stage C: strategies, statistics guard, properties V-1..V-7,
  negative-control transcripts NC-1..NC-7.
- [ ] EP-M3 / Stage D: TODO removal, roadmap 5.1.4a entry ticked,
  architecture doc sentence, developers-guide subsection, gates green.

## Surprises & discoveries

- Observation: object key-order invariance — the headline "generated key
  ordering" clause of the original TODO — is untestable in this build.
  Evidence: `Cargo.lock` shows `serde_json` 1.0.150 without `indexmap`;
  `Value::Object` is `BTreeMap`-backed, so key order is normalized before
  the code under test runs.
  Impact: the plan replaces that clause with the idempotence property (V-4)
  and a documented exclusion; recorded pre-approval so reviewers see the
  deviation from the TODO's literal wording.
- Observation: `Number::as_f64` cannot return `None` in this build, making
  the fallback branch in `round_coordinate` dead code and large-integer
  coordinates silently lossy.
  Evidence: pinned `serde_json-1.0.150/src/number.rs` lines 162–171.
  Impact: no property pins the lossy behaviour (documented exclusion); the
  dead branch is left alone (test-only change).

## Decision log

- Decision: verify with `proptest` only; no `kani` or `verus` obligations.
  Rationale: no new invariant or lemma is introduced; the domain is
  float-heavy and hash-based, where bounded model checking of `sha2` is
  intractable and a proof would restate the implementation. Property tests
  with seeded-fault non-vacuity evidence give proportionate rigour.
  Date/Author: 2026-08-16, planning agent.
- Decision: do not add `googletest` assertions despite the general testing
  brief. Rationale: `googletest` is not a dependency of the `backend` crate;
  adding it violates the no-new-dependencies constraint, and proptest bodies
  must use `prop_assert*` to preserve shrinking anyway.
  Date/Author: 2026-08-16, planning agent.
- Decision: no new `insta` snapshots and no new `rstest-bdd` scenario.
  Rationale: no multivariant output format is introduced, and no externally
  observable workflow changes; the existing Redis-backed BDD scenario already
  covers the end-to-end contract.
  Date/Author: 2026-08-16, planning agent.
- Decision: add a `5.1.4a` checkbox to `docs/backend-roadmap.md` §5.1 even
  though the roadmap has no lettered-item precedent (the earlier `3.5.4a`
  exists only as an execplan filename). Rationale: the commissioning request
  explicitly assigned roadmap reference 5.1.4a and requires the entry to be
  markable as done on completion.
  Date/Author: 2026-08-16, planning agent.
- Decision: `docs/users-guide.md` is not updated. Rationale: the change is
  test-only with no user-visible behaviour or interface change.
  Date/Author: 2026-08-16, planning agent.
- Decision: commit `proptest-regressions/` files if produced, and promote
  shrunk failures to named `rstest` cases. Rationale: proptest
  failure-persistence convention; `.gitignore` does not exclude them and the
  repository has no contrary precedent.
  Date/Author: 2026-08-16, planning agent.

## Outcomes & retrospective

To be completed as milestones land.

## Artefacts and notes

Negative-control transcripts NC-1..NC-7 are recorded here during Stage C.
(None yet; plan is in DRAFT.)
