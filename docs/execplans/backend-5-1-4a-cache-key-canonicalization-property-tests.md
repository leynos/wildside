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
collapse, order preservation for non-canonicalized arrays, divergence under
single-leaf edits, and the `route:v1:<64-character lowercase hexadecimal
SHA-256>` key format. It also adds one known-answer example test that pins
the digest algorithm itself, closing a gap where no test would fail if the
hash function were swapped.

There is no user-visible behaviour change. The observable outcome for a
developer is: `cargo test -p backend cache_key` runs the new property suite,
each property demonstrably fails when the canonicalization logic is broken
(mutation-testing and seeded-fault evidence is recorded below), and the TODO
comment is gone.

This plan was reviewed pre-implementation by a six-lens design panel; the
verdict was "proceed with conditions" and every condition is folded into the
obligations and stages below (see `Decision Log`).

## Context and orientation

Wildside is a Rust backend organized hexagonally: domain code under
`backend/src/domain/` defines ports; adapters under `backend/src/outbound/`
implement them. Route plans are cached in Redis under canonical keys so that
semantically equivalent requests share one cache entry.

Key locations:

- `backend/src/domain/ports/cache_key.rs` (377 lines) — the canonicalization
  seam. `RouteCacheKey::for_route_request(payload: &serde_json::Value)`
  normalizes the payload via the private `normalize_route_request_value`,
  hashes the compact serialization of the normalized value with
  `sha2::Sha256::digest` directly (in the private `hash_route_request_value`,
  lines 117–126; `crate::domain::idempotency::PayloadHash` is used only as a
  byte container via `from_bytes`/`to_hex` — `canonicalize_and_hash` from the
  idempotency module is **not** called on this path), and formats
  `route:v1:<hex digest>`. The inline `#[cfg(test)] mod tests` (lines
  192–377) holds nine `rstest`/`insta` test functions, which expand to
  eighteen test cases under `rstest` parameterization.
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
    zero is canonicalized to positive zero. When `Number::from_f64` on the
    rounded result fails, the original number is kept. That fallback **is**
    reachable: for `|value|` large enough that `value * 100_000.0` overflows
    to infinity (roughly `|value| > 1.8e303`), `from_f64(inf)` returns `None`
    and the original number passes through unrounded.
  - Values under coordinate keys that are not numbers (strings, objects,
    arrays, booleans, null) pass through untouched.
- `backend/src/domain/idempotency/payload.rs` — `PayloadHash` (the SHA-256
  digest container) and, separately, `canonicalize_and_hash` used by the
  idempotency feature (not by the cache-key path).
- `backend/tests/route_cache_key_canonicalization_bdd.rs` with
  `backend/tests/features/route_cache_key_canonicalization.feature` — one
  Redis-backed `rstest-bdd` scenario proving two fixed, semantically
  equivalent payloads share a cache slot. It remains unchanged.
- Module-split precedents: `backend/src/outbound/queue/apalis_route_queue.rs`
  declares `#[cfg(test)] mod tests;` resolving to
  `apalis_route_queue/tests.rs`, which declares `mod properties;`
  (`tests/properties.rs`). `backend/src/domain/jobs/enrichment/tests/
  bounding_box.rs` is a themed grandchild property file — the closest
  precedent for what this plan builds. `proptest!` is used in six files
  today; none overrides `ProptestConfig`.
- Mutation testing already runs nightly over this file:
  `.github/workflows/mutation-testing.yml` invokes the shared
  `mutation-cargo.yml` with `paths: "backend/,crates/,tools/"`, and
  `cargo-mutants` (27.x) is installed locally. This plan uses a scoped
  cargo-mutants run as its standing non-vacuity mechanism.

Facts established during planning that shape this design:

- `serde_json` is compiled without the `preserve_order` feature (confirmed
  from `Cargo.lock`: no `indexmap` dependency on `serde_json` 1.0.150), so
  `serde_json::Value::Object` is backed by `BTreeMap` and object key order is
  already sorted at the `Value` representation level. A property asserting
  "object key insertion order does not affect the key" is therefore vacuous:
  it cannot fail even if the explicit sort in
  `normalize_route_request_value` were deleted. See `Decision Log`.
  (Duplicate keys in JSON text collapse last-wins at parse time, upstream of
  this seam's `&Value` boundary, so no textual-ordering path survives.)
- `serde_json::Number::as_f64` in this build always returns `Some` (verified
  against the pinned 1.0.150 sources: `PosInt`/`NegInt` are lossily cast with
  `as f64`), so integer coordinates beyond 2^53 lose precision silently.
- Non-finite floats (NaN, infinities) cannot be constructed through the
  public `serde_json::Value` API (`Number::from_f64` rejects them) nor parsed
  from JSON text, so generators need not (and cannot) cover them. Overflow to
  infinity can still occur *inside* `round_coordinate` for finite inputs of
  extreme magnitude, as noted above.
- Double-rounding is not exactly idempotent for extreme magnitudes: when
  `|value| * 100_000.0` exceeds roughly 2^51, a second normalization pass can
  land on a neighbouring representable `f64`. Realistic coordinates are
  bounded by ±180; the property-test generators therefore bound numbers
  under coordinate keys to ±1,000,000 (a documented domain exclusion, not a
  filter).
- `proptest = "1"` is already a dev-dependency (`backend/Cargo.toml:92`). No
  new dependencies are required.
- No `proptest-regressions/` directories exist yet and `.gitignore` does not
  exclude them; the proptest failure-persistence convention is to commit
  them. For this suite they would land under
  `backend/proptest-regressions/domain/ports/cache_key/tests/properties.txt`.

Relevant guides to read before implementing: the `proptest` skill,
`docs/rust-testing-with-rstest-fixtures.md`, `docs/rstest-bdd-users-guide.md`
(for why the existing BDD suite is left alone),
`docs/wildside-backend-architecture.md` (route caching contract; the line
anchors cited in `Conformance basis` are pre-Stage-D),
`docs/complexity-antipatterns-and-refactoring-strategies.md` (module-split
hygiene), and the `hexagonal-architecture` and `rust-unit-testing` skills.

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
  `rstest`, `insta`, and `pretty_assertions` dev-dependencies; `cargo-mutants`
  is an installed developer tool, not a Cargo dependency.
- Respect the repository's 400-line file limit (`AGENTS.md`): after the
  split, `cache_key.rs`, `cache_key/tests.rs`, and the property module must
  each stay under 400 lines. Contingency: if `tests/properties.rs`
  approaches the limit, split it into a `tests/properties/` directory
  (`strategies.rs` for generators, sibling files per invariant group),
  following the `enrichment/tests/bounding_box.rs` themed-file precedent.
- Existing tests (unit, snapshot, doctest, BDD) must continue to pass
  unmodified apart from the mechanical relocation of the unit-test module.
- Property bodies must use `prop_assert!`/`prop_assert_eq!`/`prop_assert_ne!`
  (not `panic!`/`unwrap`) so shrinking works.
- Strategies must construct valid inputs by composition; `prop_filter` and
  `prop_assume!` are only acceptable for genuinely rare rejections, never for
  structural constraints (the filtering trap).
- Before the EP-M2 commit, `git diff` against the EP-M1 commit for
  `backend/src/domain/ports/cache_key.rs` must be empty (mechanical guard
  that no negative-control mutation of production code leaks into a commit).

## Tolerances (exception triggers)

- Scope: more than 7 files touched, or more than 50 net non-test production
  lines, means stop and escalate.
- Interface: if any public API signature must change, or a private function
  must be made `pub` (beyond the module-tree visibility already granted to
  descendant test modules), stop and escalate.
- Dependencies: any new crate requirement means stop and escalate.
- Iterations: if a property is still flaky or failing after two focused
  investigate-and-fix loops, stop, record the shrunk counter-example, and
  escalate — a genuine counter-example within the documented input domain is
  a production bug and is out of scope for a test-only change.
- Runtime: if the property suite adds more than 30 seconds to
  `cargo test -p backend --lib` on the development machine, reduce case
  counts or generator depth and record the decision. (Panel arithmetic
  estimates well under one second at 256 cases per property; the tolerance
  is a backstop, not a budget.)
- Mutation survivors: if the scoped cargo-mutants run leaves survivors in
  `normalize_route_request_value`, `round_coordinate`, `should_sort_array`,
  or `is_lowercase_hex_digest` after one strengthening loop, stop and record
  the surviving mutants in `Artefacts and notes` before proceeding.

## Risks

- Risk: a property finds a real counter-example in shipped canonicalization
  behaviour within the documented domain.
  Severity: medium. Likelihood: low (the extreme-float region where
  double-rounding genuinely fails is excluded from the generator domain by
  design).
  Mitigation: phrase equivalence in terms of the implementation's contract
  (equal keys when the rounded values are equal); bound coordinate-key
  numeric leaves to ±1,000,000; if a genuine bug surfaces, stop per the
  iteration tolerance, commit the regression file, and escalate with the
  shrunk input rather than patching production code under a test-only plan.
- Risk: floating-point subtlety makes the divergence property flaky.
  Severity: low. Likelihood: low.
  Mitigation: derive both coordinates in a pair from one integer grid cell
  with offsets capped at ±0.49 grid units, so equivalence and divergence are
  decided by integer choice at generation time — no runtime check or filter
  is needed (worst-case accumulated float error is about 6×10⁻⁶ grid units
  against a 0.01 guard band, a ~1,600× margin).
- Risk: the module split breaks visibility assumptions.
  Severity: low. Likelihood: low.
  Mitigation: private items are visible to all descendant modules
  (`super::` from `tests.rs`, `super::super::` from `tests/properties.rs`);
  the split is mechanical and gate-checked in its own commit.
- Risk: the fixed-seed statistics guard breaks on a proptest version bump or
  a legitimate strategy edit (the seed-to-value stream is not stable across
  versions).
  Severity: low. Likelihood: medium over the long term.
  Mitigation: keep the threshold probabilistically slack (at least one hit
  in 512 draws); document in the guard's comment that a failure after a
  proptest bump or strategy change means re-verifying witness reach and
  re-seeding, not weakening the properties.
- Risk: a committed `proptest-regressions` seed for a since-rebounded domain
  keeps replaying an out-of-domain input.
  Severity: low. Likelihood: low.
  Mitigation: when a property's domain changes, re-validate and prune its
  regression seeds in the same commit; promoted `rstest` cases remain the
  durable record.

## Conformance basis

- `docs/backend-roadmap.md` §5.1: items 5.1.1–5.1.4 are complete; this plan
  adds and discharges item 5.1.4a (the roadmap entry is created by this
  work — see `Plan of work`). Identifier: RM-5.1.4a.
- `docs/wildside-backend-architecture.md` (line anchors pre-Stage-D):
  - lines 1574–1579 (domain model): `RouteCacheKey` owns canonical
    derivation; adapters never canonicalize.
    Identifier: ARCH-CACHEKEY-OWNERSHIP.
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
ARCH-CACHEKEY-CONTRACT -> RM-5.1.4a -> EP-M2
  -> cache_key::tests::properties::{
       theme_permutations_share_a_key,                (V-1)
       coordinates_in_one_grid_cell_share_a_key,      (V-2, V-7 zero cell)
       coordinates_in_distinct_grid_cells_diverge,    (V-3)
       normalization_is_idempotent,                   (V-4)
       derived_keys_match_route_v1_format,            (V-5)
       non_theme_array_order_is_material,             (V-6)
       non_string_theme_array_order_is_material,      (V-6)
       edited_leaves_produce_distinct_keys,           (V-8)
       route_payload_reaches_special_keys }           (non-vacuity guard)
  -> cache_key::tests::{
       negative_zero_coordinate_collapses_to_zero,    (V-7 example)
       route_request_key_matches_known_sha256_digest }(V-9)
ARCH-CACHEKEY-OWNERSHIP -> EP-M1 -> module split keeps tests inside the
  domain port module (no adapter involvement)
```

## Verification plan

The change introduces no new production invariant; it verifies existing ones.
The obligations below are the invariants the shipped implementation is
claimed to satisfy, now checked over generated input ranges. Methods are
property tests plus two example tests: the input domain (arbitrary JSON
payloads, permutations, float ranges) is far too large to enumerate, no new
lemma or contractual logic is introduced (so a Verus proof would have nothing
non-trivial to prove beyond restating the code), and the properties are not
bounded-state or concurrency problems (so Kani, loom, and state-machine
checking are disproportionate). Kani was considered for
`is_lowercase_hex_digest` exhaustion but the function is a two-line character
class check pinned by V-5 and the mutation gate; a harness would restate it.

Axioms (external interfaces treated as correct, not verified here):

- AXM-1: `sha2::Sha256` implements FIPS 180-4 SHA-256 (collision behaviour
  is not tested; "different canonical bytes give different digests" is
  assumed for the divergence properties, which is sound for test purposes
  because a collision would be a cryptographic event).
- AXM-2: `serde_json` 1.0.150 without `preserve_order`: `Value::Object` is
  `BTreeMap`-backed; `to_vec` is deterministic for a given `Value`;
  `Number::from_f64` rejects non-finite values; `Number::as_f64` always
  returns `Some` in this build (lossily for integers beyond 2^53).
- AXM-3: `proptest` 1.x generates values from the stated strategies and
  shrinks failures; default 256 cases per property. The seed-to-value stream
  is not guaranteed stable across proptest versions.
- AXM-4: `serde_json` float formatting (ryu, shortest round-trip) is
  injective on distinct finite `f64` values — distinct floats serialize to
  distinct byte strings. The divergence obligations V-3 and V-8 rest on
  AXM-4 plus AXM-1, not on determinism alone.

Obligations. Each is exercised through the public entry point
`RouteCacheKey::for_route_request` unless stated; `normalize` refers to the
private `normalize_route_request_value`, reachable from the test modules
(private items are visible to descendant modules: `super::` in `tests.rs`,
`super::super::` in `tests/properties.rs`).

- Obligation V-1 (theme permutation invariance): for every generated payload
  containing an all-string array under each of `themes`, `themeIds`, and
  `interestThemeIds` (covering duplicates and empty arrays), every
  permutation of each such array yields the same cache key.
  Method: property test. Domain: string vectors length 0..8 drawn from a
  small alphabet (to force duplicates), permuted via
  `proptest::sample::Index`; payloads place the array both at the top level
  and nested under `preferences`.
  Artefact: `backend/src/domain/ports/cache_key/tests/properties.rs`,
  `theme_permutations_share_a_key`.
  Evidence: `cargo test -p backend --lib cache_key::tests::properties`.
  Non-vacuity: generated vectors include length ≥ 2 with non-identity
  permutations (witness class); the mutation gate MUT-1 must kill
  comparator/guard mutants in `normalize_route_request_value` (the
  equivalents of inverting the sort).
- Obligation V-2 (coordinate rounding equivalence): two payloads identical
  except that a coordinate field carries different `f64` values which round
  to the same five-decimal value yield the same key. Pairs are constructed
  from one integer grid cell: draw `cell: i64` in
  `-9_000_000_000..=9_000_000_000` (covering ±90,000 degrees, comfortably
  beyond real coordinates) and two offsets in ±0.49 of one grid unit, giving
  `(cell as f64 + offset) / 100_000.0`. Because the offset cap already
  guarantees both values round back to `cell` (float drift is ~6×10⁻⁶ grid
  units against the 0.01 guard band), no runtime check or filter is used.
  Method: one property test whose body iterates over each key in
  `ROUNDED_COORDINATE_KEYS` and two shapes (top level; inside an object
  inside an array, the `waypoints` shape) — a loop inside the body, not a
  multiplied case count.
  Artefact: same file, `coordinates_in_one_grid_cell_share_a_key`.
  Non-vacuity: offsets include nonzero values of both signs; manual negative
  control NC-2 (change `COORDINATE_PRECISION_FACTOR` to `1_000_000.0`) must
  fail this property (constants are outside cargo-mutants' mutation set,
  hence manual).
- Obligation V-3 (coordinate divergence): payloads identical except for
  coordinate values drawn from different grid cells yield different keys.
  Method: property test; one generator draws `cell` and a nonzero
  `delta: i64` in `1..=1_000`, using `cell` and `cell + delta`.
  Artefact: same file, `coordinates_in_distinct_grid_cells_diverge`.
  Non-vacuity: rests on AXM-1 and AXM-4; the mutation gate must kill
  "`round_coordinate` returns a constant" mutants — and V-2 must *survive*
  those same mutants' inverses, demonstrating V-2 and V-3 are independent
  obligations, not restatements.
- Obligation V-4 (normalization idempotence): for every generated payload,
  `normalize(normalize(v)) == normalize(v)`, and — mandatorily, because
  `Value::eq` treats `-0.0 == 0.0` while the two serialize differently —
  `for_route_request(v) == for_route_request(&normalize(v))`. This catches
  residual float noise from the multiply-round-divide construction: if a
  second rounding pass changed anything, canonicalization would not be a
  projection and equivalent inputs could disagree.
  Method: property test over a general recursive JSON strategy
  (`prop_recursive`, depth ≤ 4, ≤ 8 collection items, leaves covering null,
  bool, integer, float, string) whose object-key strategy is biased to emit
  the special key names (`themes`, `lat`, `lng`, ...) with high probability.
  Numbers generated under coordinate keys are bounded to ±1,000,000: outside
  roughly `|x| * 1e5 > 2^51` double rounding is genuinely not idempotent,
  and beyond ~1.8e303 the multiply overflows to infinity and rounding is
  skipped entirely; both regions are unreachable for real coordinates
  (bounded by ±180) and are documented exclusions, enforced by generator
  construction, not by filtering.
  Artefact: same file, `normalization_is_idempotent`.
  Non-vacuity: the standing statistics guard (below) proves the strategy
  reaches the sort and round branches; manual negative control NC-4 (append
  a constant to every string during normalization) must fail idempotence
  (string-content mutations of this shape are outside cargo-mutants' set).
- Obligation V-5 (key format totality): every generated payload derives
  `Ok(key)` whose string matches `^route:v1:[0-9a-f]{64}$` (checked with
  explicit character-class assertions; no regex crate). Note
  `RouteCacheKey::new` validates only emptiness and surrounding whitespace —
  hex-digest validation lives solely in `for_route_request` via
  `is_lowercase_hex_digest` — so the property's own assertions carry this
  obligation; a `new` round-trip would not. The real content of V-5 is
  totality (no panic or error on arbitrary in-domain JSON); the format shape
  is already pinned by an existing example test.
  Method: property test over the same recursive strategy as V-4.
  Artefact: same file, `derived_keys_match_route_v1_format`.
  Non-vacuity: the mutation gate must kill mutants of
  `is_lowercase_hex_digest` and of the namespace formatting (production-side
  mutations; the earlier idea of uppercasing a digit in the test harness was
  rejected as a tautological, test-side control).
- Obligation V-6 (order is material where canonicalization is not claimed):
  permuting an array of ≥ 2 pairwise-distinct elements with a non-identity
  permutation changes the key, in two variants: (a) all-string arrays under
  a key not in `SORTED_ARRAY_KEYS`; (b) arrays under a theme key containing
  at least one non-string element (the all-string guard leaves them
  unsorted). Distinctness is by construction (indexed strings `item-0`,
  `item-1`, ..., plus one distinct non-string element for variant (b));
  the permutation is rotation by one of a vector of length ≥ 2, which on
  pairwise-distinct elements is never the identity — this matters because a
  periodic array such as `[1, 1]` rotates to itself and would fail the
  property spuriously.
  Method: property tests.
  Artefacts: same file, `non_theme_array_order_is_material` and
  `non_string_theme_array_order_is_material`.
  Non-vacuity: the mutation gate must kill "sort every array
  unconditionally" mutants (e.g. `should_sort_array -> true`,
  all-string guard removed).
- Obligation V-7 (negative zero collapse): any coordinate value that rounds
  to zero (offsets within the zero grid cell, both signs) produces the same
  key as literal `0.0`.
  Method: the `cell == 0` class of V-2's generator, plus an explicit
  `rstest` example for the `json!(-0.0)` literal (proptest floats rarely
  emit signed zero).
  Artefact: V-2's property plus
  `cache_key/tests.rs::negative_zero_coordinate_collapses_to_zero`.
  Non-vacuity: the mutation gate must kill removal of the `rounded == 0.0`
  collapse (a `==`→`!=` operator mutant, within cargo-mutants' set).
- Obligation V-8 (single-leaf divergence — general injectivity guard):
  editing exactly one non-canonicalized leaf of a generated payload
  (changing a string value to a distinct string, flipping a boolean, or
  inserting a fresh key) changes the key. Without this, a normalizer that
  dropped or rewrote non-special fields would pass V-1, V-4, V-5, and V-6.
  Method: property test; the strategy picks a payload and one edit whose
  changed-ness is guaranteed by construction (e.g. appending a suffix to a
  string, inserting under a key name reserved by the generator as never
  otherwise emitted).
  Artefact: same file, `edited_leaves_produce_distinct_keys`.
  Non-vacuity: rests on AXM-1/AXM-4; the mutation gate must kill
  "normalize returns a constant/identity-erasing" mutants (e.g. replacing
  `normalize_route_request_value`'s object branch with `Value::Null`).
- Obligation V-9 (digest algorithm known-answer): one fixed payload maps to
  a precomputed `route:v1:<expected 64-hex SHA-256>` string, computed once
  from the canonical bytes with an independent tool (`sha256sum`) and pinned
  as a literal. This is the only test that fails if the hash algorithm or
  the pre-hash serialization is swapped; shape-only checks pass for any
  64-hex digest. The digest is not re-derived in test code (that would
  re-implement the function under test); it is a constant with a comment
  recording the exact command that produced it.
  Method: example test (`rstest`).
  Artefact: `cache_key/tests.rs::route_request_key_matches_known_sha256_digest`.
  Non-vacuity: fails by construction for any algorithm or serialization
  change; the mutation gate's `hash_route_request_value` mutants must be
  killed by this test.

Standing non-vacuity mechanisms (in the committed suite, not one-off):

- Statistics guard `route_payload_reaches_special_keys`: draws 512 values
  from `route_payload()` on a fixed seed and asserts at least one payload
  contains a theme key with an all-string array of length ≥ 2 and at least
  one coordinate key with a fractional value. The "at least one" threshold
  is deliberately slack (with a strategy biased toward special keys, the
  miss probability is astronomically small) so proptest version bumps or
  strategy edits do not flip it; its doc comment states that a failure after
  such a change means re-verifying witness reach and re-seeding, not
  loosening properties. Implementation sketch (the supported proptest 1.x
  sampling API):

  ```rust
  let mut runner = proptest::test_runner::TestRunner::deterministic();
  let strategy = route_payload();
  let mut saw_sortable_themes = false;
  let mut saw_fractional_coordinate = false;
  for _ in 0..512 {
      let value = strategy
          .new_tree(&mut runner)
          .expect("strategy should produce a tree")
          .current();
      saw_sortable_themes |= has_sortable_theme_array(&value);
      saw_fractional_coordinate |= has_fractional_coordinate(&value);
  }
  assert!(saw_sortable_themes && saw_fractional_coordinate);
  ```

- Mutation gate MUT-1: a scoped run of
  `cargo mutants --file backend/src/domain/ports/cache_key.rs -- --lib`
  executed at the end of Stage C and again at EP-M3. The nightly
  mutation-testing workflow keeps this evidence standing thereafter.
  cargo-mutants generates the comparator swaps, guard-function
  `-> true`/`-> false` replacements, `round_coordinate -> constant`,
  `==`→`!=` operator flips, and function-body erasures that the earlier
  draft scripted as manual controls NC-1, NC-3, NC-5, NC-6, and NC-7; it is
  physically incapable of committing a mutation and produces machine-readable
  transcripts.

- Manual negative controls (only what mutants cannot generate): NC-2
  (constant change to `COORDINATE_PRECISION_FACTOR`) and NC-4 (string-append
  inside normalization). Procedure: apply the mutation, run the focused
  property and record the shrunk failure into `Artefacts and notes`
  including the "N passed; M failed" counts (a transcript showing zero
  filtered-in tests is vacuous and must be treated as a failed control),
  then revert with `git checkout -- backend/src/domain/ports/cache_key.rs`
  and confirm `git diff --exit-code -- backend/src/domain/ports/cache_key.rs`
  before re-running to green. The property module's doc comment points at
  this ExecPlan as the record of the negative-control evidence.

Deliberately not verified, with rationale:

- Object key-order invariance: vacuous in this build (AXM-2, BTreeMap); it
  cannot fail whatever the implementation does. Recorded here and in the
  module documentation instead of writing a tautological property. If
  `preserve_order` is ever enabled, this decision must be revisited — a note
  to that effect goes into the property module's doc comment.
- Large-integer precision loss (`i64` beyond 2^53 under `lat`) and the
  extreme-magnitude regions where double rounding is non-idempotent or the
  multiply overflows: current behaviour there is an accident of `f64`
  arithmetic, real coordinates are bounded by ±180, and pinning it would
  freeze the accident. Documented as generator domain bounds (±1,000,000
  under coordinate keys) in the module doc comment; no property pins the
  excluded regions.
- SHA-256 collision resistance and ryu formatting internals: axioms AXM-1
  and AXM-4; third-party internals are not verified here. V-9 pins the
  *choice* of algorithm and serialization at the boundary, which is
  repository-owned.

## Plan of work

Stage A (no code changes): confirm the recon findings still hold
(`leta show normalize_route_request_value`, `leta show round_coordinate`;
`cargo tree -p backend -i serde_json` re-confirming no `preserve_order`);
record the pre-split baseline test count from a fresh
`cargo test -p backend --lib cache_key` run (expected: the nine test
functions expand to eighteen cases; record the actual number in
`Artefacts and notes` — EP-M1 acceptance compares against this recorded
count, not a number written into the plan).

Stage B (mechanical split, first plateau):

1. In `backend/src/domain/ports/cache_key.rs`, replace
   `#[cfg(test)] mod tests { ... }` (lines 192–377) with
   `#[cfg(test)] mod tests;`. The TODO comment moves with the module and is
   deleted only in Stage D.
2. Create `backend/src/domain/ports/cache_key/tests.rs` containing the
   existing example tests verbatim (`super::` paths are unchanged for a
   child file module).

Stage C (properties, statistics guard, mutation gate): add
`backend/src/domain/ports/cache_key/tests/properties.rs`, declared via
`mod properties;` in `tests.rs`.

1. Strategy helpers first: `theme_array()` (small-alphabet string vectors),
   `grid_cell_pair()` (integer cell plus two in-cell offsets, cap ±0.49),
   `divergent_cells()` (cell plus nonzero delta), `route_payload()`
   (recursive JSON via `prop_recursive`, key names biased toward the special
   keys, numbers under coordinate keys bounded to ±1,000,000), a
   `rotate_by_one` helper for V-6, and a single-edit generator for V-8.
   Free functions returning `impl Strategy<Value = T>`, matching the
   `generate_route/tests.rs` house style; no `prop_compose!` (no repository
   precedent). Property names deliberately omit the `route_request_key_`
   prefix used by the example tests — the `properties` module path
   disambiguates, and the shorter names keep lines inside budget; do not
   "helpfully" add the prefix.
2. The statistics guard `route_payload_reaches_special_keys` per the sketch
   in `Verification plan`.
3. Properties V-1 through V-8 as named in the trace links, using
   `proptest! { #[test] ... }` blocks with `prop_assert_eq!`/
   `prop_assert_ne!` and default configuration (no `ProptestConfig`
   override, matching all six existing proptest call sites).
4. Example tests in `tests.rs`: the `-0.0` collapse example (V-7) and the
   known-answer digest test (V-9). Compute the V-9 constant by serializing
   the normalized fixed payload and piping through `sha256sum`; record the
   command beside the constant.
5. Red evidence: production code is already believed correct, so the red
   stage is supplied by seeded faults. Run the manual controls NC-2 and
   NC-4 with the stash-bracketed procedure from `Verification plan`, then
   run the mutation gate MUT-1 and record the kill list. A property whose
   corresponding mutants survive is vacuous and must be rewritten before
   proceeding.
6. Before committing: `set -o pipefail` in the shell;
   `git diff --exit-code $EP_M1_SHA -- backend/src/domain/ports/cache_key.rs`
   must pass; `git status --porcelain` must be inspected so any generated
   `backend/proptest-regressions/` file is deliberately committed or its
   absence confirmed.

Stage D (documentation and closure):

1. Delete the TODO lines from `cache_key/tests.rs` module docs; extend the
   property module's doc comment with the documented exclusions (key-order
   vacuity under BTreeMap; coordinate-key domain bounds) and the pointer to
   this ExecPlan for the negative-control record.
2. `docs/backend-roadmap.md`: insert under §5.1, after 5.1.4:
   `- [ ] 5.1.4a. Add property-based tests for cache key canonicalization`
   `invariants (theme permutation invariance, coordinate rounding`
   `equivalence and divergence, normalization idempotence, single-leaf`
   `divergence, key format, known-answer digest).`
   It is ticked to `[x]` only when this plan reaches COMPLETE.
3. `docs/wildside-backend-architecture.md` route-caching section: one
   sentence noting the canonicalization contract is enforced by
   example-based, property-based, and mutation-tested coverage in
   `backend/src/domain/ports/cache_key/tests/`.
4. `docs/developers-guide.md`: (a) add a property-testing bullet to the
   "Testing strategy" list at the top (currently unit, integration,
   behavioural — leaving it at three kinds would contradict the new
   subsection); (b) a short subsection under testing conventions recording:
   property modules live in `tests/properties.rs` child modules, strategies
   are free functions composing valid values (no structural filtering),
   bodies use `prop_assert*`, `proptest-regressions/` files are committed
   and pruned when a property's domain changes, shrunk failures are promoted
   to named `rstest` cases, and scoped `cargo mutants --file` runs are the
   standing non-vacuity check for property suites.
5. `docs/users-guide.md`: no change — no user-visible behaviour change;
   recorded in `Decision Log` rather than editing the guide.
6. If any regression files were produced, commit them and promote the shrunk
   inputs to named `rstest` cases in `tests.rs`.

Test-framework applicability, per the repository brief: `rstest` carries the
example-shaped checks (statistics guard harness, `-0.0` literal, known-answer
digest); `proptest` carries the invariants; `insta` snapshots are not
applicable (no new multivariant output format); `googletest` is not a
dependency of this crate (see `Decision Log`); no new `rstest-bdd` scenario
is added because no externally observable workflow changes (the existing
Redis BDD scenario covers the end-to-end contract); `kani`/`verus` are
disproportionate for the reasons given in the `Verification plan`.

## Milestones and plateaus

- EP-M1 (mechanical test-module split). Outcome: `cache_key.rs` shrinks to
  ~193 production lines plus `#[cfg(test)] mod tests;`; all existing tests
  pass unchanged. Acceptance evidence: `cargo test -p backend --lib
  cache_key` passes with exactly the baseline count recorded in Stage A;
  `make check-fmt` and `make lint` show no new findings. Conformance check:
  no public interface, dependency, or format change; ARCH-CACHEKEY-OWNERSHIP
  untouched. Recovery: single mechanical commit; revert restores the inline
  module. Remaining gaps: properties. Compatibility decision: none required
  (internal test surface).
- EP-M2 (property suite green with non-vacuity evidence). Outcome:
  `properties.rs` in place; V-1..V-8 passing; V-7/V-9 examples in
  `tests.rs`; statistics guard passing; MUT-1 kill list and NC-2/NC-4
  transcripts recorded in this document; the `git diff --exit-code` guard
  against the EP-M1 commit passed immediately before committing.
  Acceptance evidence: the focused property run reports the expected test
  count (assert via `grep 'test result: ok'` on the tee'd log, under
  `set -o pipefail`; a filter matching zero tests is a failure, not a
  pass). Conformance check: ARCH-CACHEKEY-CONTRACT verified over generated
  domains; production diff empty. Recovery: properties are additive;
  reverting the commit restores EP-M1. Remaining gaps: docs.
- EP-M3 (documentation, roadmap, TODO removal). Outcome: TODO gone, roadmap
  item 5.1.4a present and ticked, architecture and developers' guides
  updated, second MUT-1 run recorded, all gates green. Acceptance evidence:
  `make check-fmt`, `make lint`, `make test` logs show no new failures
  (compare the lint log against a baseline log captured from `main` rather
  than eyeballing — the local Whitaker 0.2.7 vs CI 0.2.6 skew produces known
  noise); `git grep "TODO: Add property-based tests"
  backend/src/domain/ports` returns nothing. Recovery: docs-only commit,
  trivially revertable.

## Concrete steps

All commands run from the repository root with `set -o pipefail` active in
the shell (the `| tee` pipes otherwise mask non-zero exits). Long outputs go
through `tee` to
`/tmp/$ACTION-wildside-backend-5-1-4a-cache-key-canonicalization-property-tests.out`.
Gates run sequentially, never in parallel; full-gate runs are delegated to
the `scrutineer` subagent where possible.

```bash
set -o pipefail

# Stage A
cargo tree -p backend -i serde_json | head -20   # expect no indexmap parent
cargo test -p backend --lib cache_key 2>&1 \
  | tee /tmp/test-wildside-b514a-baseline.out
grep 'test result:' /tmp/test-wildside-b514a-baseline.out
# record the passed count in Artefacts and notes (expected 18)

# Stage B
cargo test -p backend --lib cache_key 2>&1 \
  | tee /tmp/test-wildside-b514a-m1.out
grep 'test result:' /tmp/test-wildside-b514a-m1.out
# expect: identical passed count to the recorded baseline
EP_M1_SHA=$(git rev-parse HEAD)   # after the EP-M1 commit

# Stage C (focused loop)
cargo test -p backend --lib cache_key::tests::properties 2>&1 \
  | tee /tmp/test-wildside-b514a-m2.out
grep 'test result:' /tmp/test-wildside-b514a-m2.out
# expect: 9 passed (8 properties + statistics guard); 0 passed means the
# filter matched nothing and is a FAILURE

# Stage C mutation gate (also rerun at EP-M3)
cargo mutants --file backend/src/domain/ports/cache_key.rs -- --lib 2>&1 \
  | tee /tmp/mutants-wildside-b514a.out
# expect: caught mutants for normalize_route_request_value,
# round_coordinate, should_sort_array/should_round_coordinate,
# is_lowercase_hex_digest, hash_route_request_value; record survivors

# Stage C pre-commit guards
git diff --exit-code "$EP_M1_SHA" -- backend/src/domain/ports/cache_key.rs
git status --porcelain   # inspect for untracked proptest-regressions files

# Stage D / gates (delegate to scrutineer; sequential)
make check-fmt 2>&1 | tee /tmp/check-fmt-wildside-b514a.out
make lint      2>&1 | tee /tmp/lint-wildside-b514a.out
make test      2>&1 | tee /tmp/test-wildside-b514a.out
```

Note: `make check-fmt` and `make lint` are known to exit 0 even when
sub-checks fail; read the tee'd logs, not the exit codes. `make lint` may
show pre-existing Whitaker suite findings caused by a local 0.2.7 vs CI
0.2.6 version skew; diff against a `main` baseline log so only newly
introduced findings block progress.

Commit after each stage (Stage B, Stage C, each Stage D document), using
file-based commit messages per the `commit-message` skill.

## Validation and acceptance

Acceptance is behavioural:

1. Before Stage C, `cargo test -p backend --lib cache_key::tests::properties`
   reports zero matching tests (module absent). After Stage C it reports the
   eight named properties plus the statistics guard, all passing, and
   `cache_key::tests` additionally reports the `-0.0` and known-answer
   examples.
2. Red evidence: the MUT-1 transcript shows the expected mutant kills; the
   NC-2 and NC-4 transcripts show the focused property failing with a
   shrunk counter-example and a nonzero filtered-in test count while the
   mutation is applied, and passing after a verified-clean revert. Example
   expected shape for NC-2:

   ```plaintext
   Test failed: assertion failed: `(left == right)` ... minimal failing
   input: cell = 1, offsets = (0.0, 0.4)
   running 1 test ... test result: FAILED. 0 passed; 1 failed
   ```

3. `make check-fmt`, `make lint`, and `make test` logs show no findings
   beyond the `main` baseline.
4. `docs/backend-roadmap.md` contains the ticked 5.1.4a entry; the TODO
   comment is gone from the ports module; the developers' guide lists
   property testing in its testing-strategy overview.

## Idempotence and recovery

Every stage is an additive or mechanical commit; re-running any command is
safe. If a property fails intermittently, the committed
`proptest-regressions/` seed makes the failure reproducible (stored seeds
replay first). When a property's generator domain changes, re-validate and
prune its regression seeds in the same commit. Rollback at any plateau is
`git revert` of that stage's commit; no data, schema, or wire format is
touched. Manual negative controls are bracketed by
`git checkout -- backend/src/domain/ports/cache_key.rs` plus
`git diff --exit-code`, so an interrupted control run is recovered by
running those two commands.

## Interfaces and dependencies

No interface changes. The property module consumes:

- `RouteCacheKey`, `RouteCacheKeyDerivationError`, and
  `RouteCacheKeyValidationError` plus the private items
  `normalize_route_request_value`, `SORTED_ARRAY_KEYS`,
  `ROUNDED_COORDINATE_KEYS`, and `COORDINATE_PRECISION_FACTOR`, all via
  `super::super::` (private items are visible to descendant test modules;
  prefer the `super::` chain over crate paths, matching the
  `apalis_route_queue` precedent);
- dev-dependencies already present: `proptest = "1"`, `rstest = "0.26"`,
  `serde_json` (`json!` macro), `pretty_assertions` where a non-property
  equality assertion reads better;
- developer tooling: `cargo-mutants` (installed binary; also run nightly by
  `.github/workflows/mutation-testing.yml`).

## Progress

- [ ] Stage A: re-confirm recon facts; record baseline test count.
- [ ] EP-M1 / Stage B: split `cache_key.rs` tests into `cache_key/tests.rs`.
- [ ] EP-M2 / Stage C: strategies, statistics guard, properties V-1..V-8,
  V-7/V-9 examples.
- [ ] EP-M2: mutation gate MUT-1 run and kill list recorded.
- [ ] EP-M2: manual control NC-2 transcript recorded and revert verified.
- [ ] EP-M2: manual control NC-4 transcript recorded and revert verified.
- [ ] EP-M2: pre-commit guards (`git diff --exit-code` vs EP-M1;
  `git status --porcelain` reviewed).
- [ ] EP-M3 / Stage D: TODO removal, roadmap 5.1.4a entry ticked,
  architecture doc sentence, developers-guide bullet and subsection, second
  MUT-1 run, gates green.

## Surprises & discoveries

- Observation: object key-order invariance — the headline "generated key
  ordering" clause of the original TODO — is untestable in this build.
  Evidence: `Cargo.lock` shows `serde_json` 1.0.150 without `indexmap`;
  `Value::Object` is `BTreeMap`-backed, so key order is normalized before
  the code under test runs.
  Impact: the plan replaces that clause with the idempotence property (V-4)
  and a documented exclusion; recorded pre-approval so reviewers see the
  deviation from the TODO's literal wording.
- Observation: `Number::as_f64` cannot return `None` in this build, but the
  `Number::from_f64` fallback in `round_coordinate` *is* reachable — for
  finite inputs of extreme magnitude the multiply overflows to infinity and
  the original number passes through unrounded.
  Evidence: pinned `serde_json-1.0.150/src/number.rs` lines 162–171;
  `round_coordinate` arithmetic at `cache_key.rs:173–182`.
  Impact: generator domains under coordinate keys are bounded to ±1,000,000
  so V-4 cannot manufacture a counter-example in input space no real
  request occupies (double rounding is also genuinely non-idempotent above
  roughly `|x| * 1e5 > 2^51`).
- Observation (design panel): no existing or previously planned test would
  fail if the hash algorithm or pre-hash serialization were swapped —
  `cache_key.rs` calls `Sha256::digest` directly and the example tests check
  only digest shape.
  Evidence: `hash_route_request_value` at `cache_key.rs:117–126`;
  `route_request_key_has_expected_namespace_and_hash_shape` checks
  length/hex only.
  Impact: obligation V-9 (known-answer test) added; Context wording
  corrected (the plan previously claimed the path went through
  `canonicalize_and_hash`).
- Observation (design panel): the repository already runs nightly
  cargo-mutants over `backend/`, making hand-scripted mutation controls
  mostly redundant.
  Evidence: `.github/workflows/mutation-testing.yml`; `cargo-mutants` 27.x
  installed locally.
  Impact: the manual NC-1/NC-3/NC-5/NC-6/NC-7 controls from the first draft
  were replaced by the scoped, repeatable MUT-1 gate; only NC-2 (constant
  edit) and NC-4 (string append) remain manual because cargo-mutants does
  not generate those mutation shapes.

## Decision log

- Decision: verify with `proptest` plus two `rstest` examples and a scoped
  `cargo-mutants` gate; no `kani` or `verus` obligations.
  Rationale: no new invariant or lemma is introduced; the domain is
  float-heavy and hash-based, where bounded model checking of `sha2` is
  intractable and a proof would restate the implementation. Property tests
  with mutation-gate non-vacuity evidence give proportionate rigour.
  Date/Author: 2026-08-16, planning agent.
- Decision: adopt the design panel's amendments — add V-8 (single-leaf
  divergence) and V-9 (known-answer digest); bound coordinate-key numeric
  generation to ±1,000,000; replace five manual negative controls with the
  MUT-1 mutation gate; make the no-mutation-committed check an executable
  `git diff --exit-code` step; require `set -o pipefail` and scripted count
  assertions on all tee'd verification pipes; add AXM-4 (ryu injectivity).
  Alternatives considered and rejected: an equivalence-oracle reference
  canonicalizer (mirrors the implementation's mental model — the shared-
  misconception anti-pattern — and cannot express divergence obligations);
  restructuring around a single perturbation-enum property (muddles failure
  attribution and trace links); an optional composed-perturbations property
  (deferred — the single-edit and per-invariant properties cover the risk
  proportionately; revisit if a real interaction bug ever surfaces).
  Date/Author: 2026-08-16, planning agent, on panel review.
- Decision: do not add `googletest` assertions despite the general testing
  brief. Rationale: `googletest` is not a dependency of the `backend` crate;
  adding it violates the no-new-dependencies constraint, and proptest bodies
  must use `prop_assert*` to preserve shrinking anyway.
  Date/Author: 2026-08-16, planning agent.
- Decision: no new `insta` snapshots and no new `rstest-bdd` scenario.
  Rationale: no multivariant output format is introduced, and no externally
  observable workflow changes; the existing Redis-backed BDD scenario
  already covers the end-to-end contract.
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
- Decision: commit `proptest-regressions/` files if produced, promote shrunk
  failures to named `rstest` cases, and prune seeds whenever a property's
  generator domain changes. Rationale: proptest failure-persistence
  convention; stale seeds against a rebounded domain would keep CI red or
  silently stop testing the original counter-example.
  Date/Author: 2026-08-16, planning agent.

## Outcomes & retrospective

To be completed as milestones land.

## Artefacts and notes

To be populated during implementation: the Stage A baseline test count, the
MUT-1 kill lists (Stage C and EP-M3), and the NC-2/NC-4 transcripts with
their filtered-in test counts.

______________________________________________________________________

Revision note (2026-08-16): revised after the six-lens design panel review.
Added obligations V-8 (single-leaf divergence) and V-9 (known-answer digest);
corrected the false claim that the cache-key path uses
`canonicalize_and_hash`; corrected the `from_f64`-fallback reachability
claim and bounded coordinate-key generation to ±1,000,000 to keep V-4 out of
the genuinely non-idempotent extreme-float region; replaced manual controls
NC-1/NC-3/NC-5/NC-6/NC-7 with the scoped cargo-mutants gate MUT-1 (the
nightly mutation workflow already covers this file); fixed the V-5 claim
that `RouteCacheKey::new` re-validates hex format (it does not); added
AXM-4 (ryu injectivity) underpinning the divergence obligations; specified
distinctness-by-construction and rotation-by-one for V-6 (periodic arrays
would otherwise fail spuriously); made the no-mutation-committed check an
executable `git diff --exit-code` step and added `set -o pipefail` plus
scripted count assertions; corrected the baseline test count to a
recorded-at-Stage-A value (the rstest functions expand to eighteen cases,
not twelve); added the statistics-guard implementation sketch and its
version-brittleness caveat; renamed `distinct_grid_cells_diverge` to
`coordinates_in_distinct_grid_cells_diverge` and
`non_string_theme_arrays_preserve_order` to
`non_string_theme_array_order_is_material`; added the 400-line contingency
(split `properties/` directory) and the developers-guide testing-strategy
bullet. Remaining work is unchanged in structure: split, properties, docs.
