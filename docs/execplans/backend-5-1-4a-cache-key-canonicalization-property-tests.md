# Add property-based tests for route cache key canonicalization invariants

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
`Outcomes & Retrospective`, `Conformance Basis`, and `Verification Plan` must
be kept up to date as work proceeds.

Status: IN PROGRESS

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
equivalence and divergence, normalization idempotence, negative-zero collapse,
order preservation for non-canonicalized arrays, divergence under single-leaf
edits, and the `route:v1:<64-character lowercase hexadecimal SHA-256>` key
format. It also adds one known-answer example test that pins the digest
algorithm itself, closing a gap where no test would fail if the hash function
were swapped.

The canonicalization code is not yet called by any production path (its only
caller outside its own module is the Redis-backed behavioural test), so there
are no deployed cache entries or clients to protect. Counter-examples are
therefore treated as authoritative: **a property failure inside the natural
input domain is a production defect to be fixed, not an input region to be
fenced off by narrowing the generator.** Planning has already found three such
defects in coordinate rounding (see `Surprises & Discoveries`); this plan fixes
them by bounding the admitted coordinate magnitude in the domain canonicalizer,
rejecting out-of-range coordinates with a typed error, and establishing the
rounding projection property with a hand proof, generated properties, and an
exhaustive test over a dense representable-float slice near the bound. The
planned Kani proof was evaluated and declined: the installed guidance says it
does not model floating-point precision, and both 30-minute spikes stalled
before a verdict.

The observable outcome for a developer is: `cargo test -p backend cache_key`
runs the new property suite over the full finite JSON number domain; each
property demonstrably fails when the canonicalization logic is broken
(mutation-testing and seeded-fault evidence is recorded below); LEM-1 is
supported by its written arithmetic argument, full-domain generated properties,
and a dense boundary-slice regression; route requests whose coordinate-keyed
numbers exceed ±180 in magnitude fail key derivation with
`RouteCacheKeyDerivationError::CoordinateOutOfRange` instead of producing
silently wrong keys; and the TODO comment is gone. Cache keys for in-range
requests are byte-for-byte unchanged.

This plan was reviewed pre-implementation by a six-lens design panel; the
verdict was "proceed with conditions" and every condition is folded into the
obligations and stages below (see `Decision Log`). It was then revised on the
commissioning engineer's instruction that counter-examples carry more authority
than the first revision gave them.

## Context and orientation

Wildside is a Rust backend organized hexagonally: domain code under
`backend/src/domain/` defines ports; adapters under `backend/src/outbound/`
implement them. Route plans are cached in Redis under canonical keys so that
semantically equivalent requests share one cache entry.

Key locations:

- `backend/src/domain/ports/cache_key.rs` (215 lines) — the canonicalization
  seam. `RouteCacheKey::for_route_request(payload: &serde_json::Value)`
  normalizes the payload via the private `normalize_route_request_value`,
  hashes the compact serialization of the normalized value with
  `sha2::Sha256::digest` directly (in the private `hash_route_request_value`,
  lines 117–126; `crate::domain::idempotency::PayloadHash` is used only as a
  byte container via `from_bytes`/`to_hex` — `canonicalize_and_hash` from the
  idempotency module is **not** called on this path), and formats
  `route:v1:<hex digest>`. Its `#[cfg(test)] mod tests` declaration (line 215)
  resolves to `backend/src/domain/ports/cache_key/tests.rs`; property modules
  live under `backend/src/domain/ports/cache_key/tests/properties/`.
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
    places via `(value * 100_000.0).round() / 100_000.0` in the private
    `round_coordinate` (lines 173–182); a result equal to zero is
    canonicalized to positive zero. When `Number::from_f64` on the rounded
    result fails, the original number is kept (reachable — see defect D-2).
  - Values under coordinate keys that are not numbers (strings, objects,
    arrays, booleans, null) pass through untouched.
- Callers: `rg for_route_request backend/src` finds no production caller;
  `backend/tests/route_cache_key_canonicalization_bdd.rs` is the only external
  user. The public error enum `RouteCacheKeyDerivationError` is re-exported from
  `backend/src/domain/ports/mod.rs:50` but matched nowhere.
- `backend/src/domain/idempotency/payload.rs` — `PayloadHash` (the SHA-256
  digest container) and, separately, `canonicalize_and_hash` used by the
  idempotency feature (not by the cache-key path).
- `backend/tests/route_cache_key_canonicalization_bdd.rs` with
  `backend/tests/features/route_cache_key_canonicalization.feature` — one
  Redis-backed `rstest-bdd` scenario proving two fixed, semantically equivalent
  payloads share a cache slot. It uses in-range coordinates and remains
  unchanged.
- Module-split precedents: `backend/src/outbound/queue/apalis_route_queue.rs`
  declares `#[cfg(test)] mod tests;` resolving to
  `apalis_route_queue/tests.rs`, which declares `mod properties;`
  (`tests/properties.rs`).
  `backend/src/domain/jobs/enrichment/tests/ bounding_box.rs` is a themed
  grandchild property file — the closest precedent for what this plan builds.
  `proptest!` is used in six files today; none overrides `ProptestConfig`.
- Mutation testing already runs nightly over this file:
  `.github/workflows/mutation-testing.yml` invokes the shared
  `mutation-cargo.yml` with `paths: "backend/,crates/,tools/"`, and
  `cargo-mutants` (27.x) is installed locally. This plan uses a scoped
  cargo-mutants run as its standing non-vacuity mechanism.
- Kani 0.67 (`cargo kani`) is installed locally; the repository has no Kani
  harnesses or wiring yet. Workspace lints live in the root `Cargo.toml`
  (`[workspace.lints.rust]`, line 72), which does not yet declare `cfg(kani)`.

Facts established during planning that shape this design:

- `serde_json` is compiled without the `preserve_order` feature (confirmed
  from `Cargo.lock`: no `indexmap` dependency on `serde_json` 1.0.150), so
  `serde_json::Value::Object` is backed by `BTreeMap` and object key order is
  already sorted at the `Value` representation level. A property asserting
  "object key insertion order does not affect the key" is therefore vacuous: it
  cannot fail even if the explicit sort in `normalize_route_request_value` were
  deleted. See `Decision Log`. (Duplicate keys in JSON text collapse last-wins
  at parse time, upstream of this seam's `&Value` boundary, so no
  textual-ordering path survives.)
- `serde_json::Number::as_f64` in this build always returns `Some` (verified
  against the pinned 1.0.150 sources: `PosInt`/`NegInt` are lossily cast with
  `as f64`).
- Non-finite floats (NaN, infinities) cannot be constructed through the
  public `serde_json::Value` API (`Number::from_f64` rejects them) nor parsed
  from JSON text, so generators need not (and cannot) cover them.
- `proptest = "1"` is already a dev-dependency (`backend/Cargo.toml:92`). No
  new dependencies are required.
- No `proptest-regressions/` directories exist yet and `.gitignore` does not
  exclude them; the proptest failure-persistence convention is to commit them.
  For this suite they would land under
  `backend/proptest-regressions/domain/ports/cache_key/tests/properties.txt`.

Known production defects (found during planning; each is a counter-example to
an obligation below and is fixed by this plan, not excluded):

- D-1 (idempotence failure at large magnitude): once `|value| * 100_000.0`
  exceeds roughly 2^51, the divide-back step is no longer exact enough for a
  second rounding pass to land on the same integer, so
  `normalize(normalize(v)) != normalize(v)` — a counter-example to V-4.
- D-2 (rounding silently skipped): for finite `|value|` above roughly
  1.8e303 the multiply overflows to infinity, `Number::from_f64(inf)` returns
  `None`, and the original number passes through unrounded — a counter-example
  to V-2 (two values that "round alike" keep distinct keys).
- D-3 (distinct integers collapse): integer coordinates beyond 2^53 are
  lossily cast by `as_f64`, so distinct `i64`/`u64` coordinate values map to
  one key — a counter-example to V-3/V-8 divergence.

All three live only at magnitudes no real coordinate occupies (latitude is
bounded by ±90, longitude by ±180). The first revision of this plan hid them by
bounding the generators; this revision fixes them at the source.

Relevant guides to read before implementing: the `proptest`, `kani`,
`rust-errors`, `rust-unit-testing`, and `hexagonal-architecture` skills;
`docs/rust-testing-with-rstest-fixtures.md`; `docs/rstest-bdd-users-guide.md`
(for why the existing BDD suite is left alone);
`docs/rust-doctest-dry-guide.md` (the `for_route_request` doctest gains an
error-path sentence); `docs/wildside-backend-architecture.md` (route caching
contract; the line anchors cited in `Conformance basis` are pre-Stage-E); and
`docs/complexity-antipatterns-and-refactoring-strategies.md` (module-split
hygiene).

## Constraints

- Production edits in `backend/src/domain/ports/cache_key.rs` are limited to:
  the mechanical test-module split; the private scalar extraction into
  `round_coordinate_value` for direct boundary-slice testing; removal of the
  TODO comment; and fixes for counter-examples to the obligations in
  `Verification plan` (the known defects D-1..D-3 and any further
  counter-example found inside the generator domain). Every production fix must
  be preceded by a recorded failing property or example (red), then land
  together with that test passing (green) in one commit.
- A production fix must not change the canonical bytes, and hence the cache
  key, of any request whose coordinate-keyed numbers are all within ±180. The
  known-answer test V-9 and the existing example tests enforce this. (No cache
  is deployed, so this is a design-hygiene constraint rather than a migration
  one; if a future fix genuinely needs to change in-range keys, that is an
  escalation, and the namespace would move to `route:v2`.)
- The permitted public-interface change is exactly one new variant,
  `RouteCacheKeyDerivationError::CoordinateOutOfRange { key: String, value:
  String }`,
  where `value` is the JSON number's textual form (an `f64` field would break
  the enum's `Eq` derive). Private function signatures inside `cache_key.rs`
  may change freely.
- Keep the canonicalization seam and its coordinate-domain policy in the
  domain; do not move logic into
  `backend/src/outbound/cache/redis_route_cache.rs` or test through the Redis
  adapter.
- No new crate dependencies (dev or production). Use the existing
  `proptest`, `rstest`, `insta`, and `pretty_assertions` dev-dependencies;
  `cargo-mutants` and `cargo-kani` are installed developer tools, not Cargo
  dependencies. Kani is evaluated for LEM-1 but not used because its installed
  guidance excludes floating-point precision and the bounded spikes timed out.
- Respect the repository's 400-line file limit (`AGENTS.md`): after the
  split, `cache_key.rs`, `cache_key/tests.rs`, and property modules must each
  stay under 400 lines. Contingency: if `tests/properties.rs` approaches the
  limit, split it into a `tests/properties/` directory (`strategies.rs` for
  generators, sibling files per invariant group), following the
  `enrichment/tests/bounding_box.rs` themed-file precedent.
- Existing tests (unit, snapshot, doctest, BDD) must continue to pass
  unmodified apart from the mechanical relocation of the unit-test module.
- Property bodies must use `prop_assert!`/`prop_assert_eq!`/`prop_assert_ne!`
  (not `panic!`/`unwrap`) so shrinking works.
- Strategies must construct valid inputs by composition; `prop_filter` and
  `prop_assume!` are only acceptable for genuinely rare rejections, never for
  structural constraints (the filtering trap). Generators must not be narrowed
  to avoid a counter-example; the only permitted domain bounds are the ones the
  production contract itself states (the ±180 admission bound after Stage C2).
- Manual negative-control mutations are never committed: before each
  commit, `git diff` of `cache_key.rs` against the intended change must show
  only the planned fix (see the pre-commit guard in `Concrete steps`).

## Tolerances (exception triggers)

- Scope: more than 12 files touched, or more than 80 net non-test production
  lines, means stop and escalate.
- Interface: any public API change other than the single
  `CoordinateOutOfRange` variant, or making a private item `pub`, means stop
  and escalate.
- Canonical form: any fix that would change the key of an in-range request
  (V-9 or an existing example test turning red) means stop and escalate.
- Dependencies: any new crate requirement means stop and escalate.
- New counter-examples: a counter-example beyond D-1..D-3 is fixed under the
  same red-green discipline if the fix fits the interface and canonical-form
  tolerances; otherwise record the shrunk input, commit nothing red, and
  escalate. Two failed fix attempts on the same counter-example also trigger
  escalation.
- Kani: if the harness cannot be built against the `backend` crate, or does
  not complete within 30 minutes or 16 GB of memory, stop Kani work and record
  the failure mode. The wrapper and isolated-kernel spikes both met this limit;
  Kani is declined for this floating-point claim. The hand proof LEM-1 and the
  full-domain properties remain in force, with a deterministic dense boundary
  slice added as the measured fallback.
- Runtime: if the property suite adds more than 30 seconds to
  `cargo test -p backend --lib` on the development machine, reduce case counts
  or generator depth and record the decision. (Panel arithmetic estimates well
  under one second at 256 cases per property; the tolerance is a backstop, not
  a budget.)
- Mutation survivors: if the scoped cargo-mutants run leaves survivors in
  `normalize_route_request_value`, `round_coordinate`, `should_sort_array`, or
  `is_lowercase_hex_digest` after one strengthening loop, stop and record the
  surviving mutants in `Artefacts and notes` before proceeding.

## Risks

- Risk: properties find counter-examples beyond D-1..D-3.
  Severity: medium. Likelihood: low-to-medium (the generators now roam the full
  finite number domain, which is the point). Mitigation: that is the desired
  outcome; fix under the red-green discipline and the tolerances above, promote
  the shrunk input to a named `rstest` case, and commit the regression file.
- Risk: rejecting out-of-range coordinates is the wrong policy (for example,
  a future caller wants totality). Severity: low. Likelihood: low. Mitigation:
  there is no caller today; the alternatives (pass-through, clamping,
  integer-grid serialization) are recorded in `Decision Log` together with why
  they were rejected. Reversing the policy is a local change behind the same
  private function.
- Risk: floating-point subtlety makes the divergence property flaky.
  Severity: low. Likelihood: low. Mitigation: derive both coordinates in a pair
  from one integer grid cell with offsets capped at ±0.49 grid units, so
  equivalence and divergence are decided by integer choice at generation time;
  at the admitted magnitude (|cell| ≤ 18,000,000) float drift is below 10⁻⁸
  grid units.
- Risk: the module split breaks visibility assumptions.
  Severity: low. Likelihood: low. Mitigation: private items are visible to all
  descendant modules (`super::` from `tests.rs`, `super::super::` from
  `tests/properties.rs`); the split is mechanical and gate-checked in its own
  commit.
- Risk: the fixed-seed statistics guard breaks on a proptest version bump or
  a legitimate strategy edit (the seed-to-value stream is not stable across
  versions). Severity: low. Likelihood: medium over the long term. Mitigation:
  keep the thresholds probabilistically slack (at least one hit per witness
  class in 512 draws); document in the guard's comment that a failure after a
  proptest bump or strategy change means re-verifying witness reach and
  re-seeding, not weakening the properties.
- Risk: a committed `proptest-regressions` seed keeps replaying an input
  whose expected outcome changed with a fix (for example a D-1 input that is
  now rejected rather than rounded). Severity: low. Likelihood: medium.
  Mitigation: re-validate and prune regression seeds in the same commit as any
  fix; promoted `rstest` cases remain the durable record.
- Risk: no machine proof is available for LEM-1 with the installed Kani tool.
  Severity: medium. Likelihood: confirmed. Evidence: its local skill says
  floating-point precision is not modelled, and wrapper and isolated-kernel
  spikes each exceeded 30 minutes without a verdict. Mitigation: do not land or
  claim an unsupported Kani proof; retain the hand argument, generated full-
  domain properties, and deterministic dense-slice regression. Revisit with a
  verifier that explicitly supports the required IEEE-754 operations.

## Conformance basis

- `docs/backend-roadmap.md` §5.1: items 5.1.1–5.1.4 are complete; this plan
  adds and discharges item 5.1.4a (the roadmap entry is created by this work —
  see `Plan of work`). Identifier: RM-5.1.4a.
- `docs/wildside-backend-architecture.md` (line anchors pre-Stage-E):
  - lines 1574–1579 (domain model): `RouteCacheKey` owns canonical
    derivation; adapters never canonicalize.
    Identifier: ARCH-CACHEKEY-OWNERSHIP.
  - lines 1938–1950 (route caching): the canonical form sorts the three theme
    arrays, rounds coordinates to five decimal places, serializes JSON with
    stable key ordering, and hashes with SHA-256; keys follow
    `route:v1:<sha256>`. Identifier: ARCH-CACHEKEY-CONTRACT. Stage E extends
    this contract with the coordinate admission bound
    (ARCH-CACHEKEY-COORD-DOMAIN).
- Prior art (external, informative only): RFC 8785 (JSON Canonicalization
  Scheme) motivates hashing a canonical JSON form; this repository uses its own
  simpler canonical form (BTreeMap key order plus compact
  `serde_json::to_vec`), not RFC 8785 number/string formatting. No conformance
  to RFC 8785 is claimed or planned.
- No Terms of Reference document exists for this work; the roadmap and the
  architecture document are the upstream artefacts.

Trace links:

```plaintext
ARCH-CACHEKEY-CONTRACT -> RM-5.1.4a -> EP-M2, EP-M3
  -> cache_key::tests::properties::{
       theme_permutations_share_a_key,                (V-1)
       coordinates_in_one_grid_cell_share_a_key,      (V-2, V-7 zero cell)
       coordinates_in_distinct_grid_cells_diverge,    (V-3)
       normalization_is_idempotent,                   (V-4)
       derived_keys_match_route_v1_format,            (V-5)
       non_theme_array_order_is_material,             (V-6)
       non_string_theme_array_order_is_material,      (V-6)
       edited_leaves_produce_distinct_keys,           (V-8)
       out_of_range_coordinates_are_rejected,         (V-10)
       route_payload_reaches_special_keys }           (non-vacuity guard)
  -> cache_key::tests::{
       negative_zero_coordinate_collapses_to_zero,    (V-7 example)
       route_request_key_matches_known_sha256_digest, (V-9)
       coordinate_bound_is_inclusive,                 (V-10 boundary)
       <promoted D-1..D-3 regression cases> }
ARCH-CACHEKEY-COORD-DOMAIN -> EP-M3
  -> cache_key::tests::dense_rounding_slice_near_coordinate_bound (LEM-1 fallback)
ARCH-CACHEKEY-OWNERSHIP -> EP-M1 -> module split keeps tests inside the
  domain port module (no adapter involvement)
```

## Verification plan

The work verifies existing invariants over generated input ranges and, because
the fixes introduce one new contractual rule (the coordinate admission bound)
and one lemma the rule exists to make true (rounding is a projection on the
admitted domain), backed by a hand proof, generated properties, and the dense
boundary-slice regression described below.

Method selection:

- Property tests carry the payload-level invariants: the input domain
  (arbitrary JSON payloads, permutations, all finite numbers) is far too large
  to enumerate, and the invariants are relational (pairs of payloads), which
  proptest expresses directly.
- LEM-1 carries a written arithmetic argument. Kani and Verus were assessed
  for this finite `f64` claim, but neither provides suitable evidence in this
  environment: the installed Kani guidance says floating-point precision is not
  modelled and two Kani spikes exceeded 30 minutes without a verdict; Verus has
  limited IEEE-754 reasoning and would require axiomatizing the rounding facts
  under test. The claim is therefore supported by the hand derivation,
  full-domain generated properties, and an exhaustive test of all representable
  values in a fixed 100,000-ULP slice inside each ±180 boundary. This slice is
  a regression check, not a proof over the whole domain. Kani is also not used
  for `is_lowercase_hex_digest` (a two-line character-class check pinned by V-5
  and the mutation gate) or payload-level properties (unbounded recursive
  structures).
- Loom and state-machine testing are inapplicable (no concurrency, no state).

Axioms (external interfaces treated as correct, not verified here):

- AXM-1: `sha2::Sha256` implements FIPS 180-4 SHA-256 (collision behaviour
  is not tested; "different canonical bytes give different digests" is assumed
  for the divergence properties, which is sound for test purposes because a
  collision would be a cryptographic event).
- AXM-2: `serde_json` 1.0.150 without `preserve_order`: `Value::Object` is
  `BTreeMap`-backed; `to_vec` is deterministic for a given `Value`;
  `Number::from_f64` rejects non-finite values; `Number::as_f64` always returns
  `Some` in this build (lossily for integers beyond 2^53).
- AXM-3: `proptest` 1.x generates values from the stated strategies and
  shrinks failures; default 256 cases per property. The seed-to-value stream is
  not guaranteed stable across proptest versions.
- AXM-4: `serde_json` float formatting (ryu, shortest round-trip) is
  injective on distinct finite `f64` values — distinct floats serialize to
  distinct byte strings.
- AXM-5: Rust `f64` arithmetic (`*`, `/`, `round`) follows IEEE-754 binary64
  semantics: multiplication and division round to nearest-even, while `round`
  rounds half away from zero. This is an assumption of the hand proof; Kani
  runs here do not validate it.

Lemma LEM-1 (rounding is a projection on the admitted domain). Let
`MAX_COORDINATE_MAGNITUDE = 180.0` and `P = 100_000.0`. For every finite `f64`
`x` with `|x| <= 180`, define `n = round(x * P)` and
`r = canonical_zero(n / P)` (the current `round_coordinate` arithmetic). Then
(a) `|r| <= 180`, so `r` is itself admitted; (b) `round(r * P) == n`; and hence
(c) `round_coordinate(r) == r` bit-for-bit. Hand proof: `P` is exactly
representable. `|x * P| <= 1.8e7 < 2^25`, so `n` is an integer with
`|n| < 2^25`, exactly representable. Division gives
`n / P = (n / 10^5)(1 + d1)` with `|d1| <= 2^-53`; multiplying back gives
`(n / P) * P = n (1 + d1)(1 + d2)` with `|d2| <= 2^-53`, so the error from `n`
is at most `|n| * (2^-52 + 2^-106) < 2^25 * 2^-51.9 < 2^-26`, far below the
`1/2` needed for `round` to return `n`; that is (b). Since `|n| <= 1.8e7`
exactly (rounding cannot overshoot the integer `1.8e7`), `|n / P| <= 180` after
correctly rounded division; that is (a). The zero case: `n == 0` gives
`r == +0.0` by the canonical-zero step, and `round(+0.0 * P) == 0`. (c) follows
because `r` is a function of `n` alone. A corollary used by V-3: `n` is
recoverable from `r` by (b), so distinct grid cells give distinct `r` and, by
AXM-4, distinct bytes. Method: hand proof plus
`dense_rounding_slice_near_coordinate_bound`, which enumerates all representable
`f64` values in the 100,000-ULP interval immediately inside each of `-180.0`
and `180.0`, asserting (a), (b), and (c). This is an exhaustive regression
check for those intervals, not a machine proof over all admitted `f64` values.
Full-domain `proptest` properties exercise generated finite values; the hand
proof carries the remaining domain argument. No Kani harness is landed: both the
`Number` wrapper spike and the isolated scalar kernel exceeded 30 minutes, and
the installed Kani guidance says it does not model floating-point precision.
Logs and resource measurements are recorded in `Artefacts and notes`; the
written disposition is in `docs/developers-guide.md`.

Obligations. Each is exercised through the public entry point
`RouteCacheKey::for_route_request` unless stated; `normalize` refers to the
private `normalize_route_request_value`, reachable from the test modules
(private items are visible to descendant modules: `super::` in `tests.rs`,
`super::super::` in `tests/properties.rs`). "Coordinate leaf" means a JSON
number whose immediate containing object key is in `ROUNDED_COORDINATE_KEYS`.

- Obligation V-1 (theme permutation invariance): for every generated payload
  containing an all-string array under each of `themes`, `themeIds`, and
  `interestThemeIds` (covering duplicates and empty arrays), every permutation
  of each such array yields the same cache key. Method: property test. Domain:
  string vectors length 0..8 drawn from a small alphabet (to force duplicates),
  permuted via `proptest::sample::Index`; payloads place the array both at the
  top level and nested under `preferences`. Artefact:
  `backend/src/domain/ports/cache_key/tests/properties.rs`,
  `theme_permutations_share_a_key`. Evidence:
  `cargo test -p backend --lib cache_key::tests::properties`. Non-vacuity:
  generated vectors include length ≥ 2 with non-identity permutations (witness
  class); the mutation gate MUT-1 must kill comparator/guard mutants in
  `normalize_route_request_value`.
- Obligation V-2 (coordinate rounding equivalence): two payloads identical
  except that a coordinate leaf carries different values which round to the
  same five-decimal grid cell yield the same key. Pairs are constructed from
  one integer grid cell: draw `cell: i64` in `-18_000_000..=18_000_000` (the
  full admitted range) and one offset magnitude in `0.01..0.49` grid units,
  then use its negative and positive forms. This guarantees distinct inputs
  without filtering. Convert with `(cell as f64 + offset) / 100_000.0`, clamped
  into ±180 only at the two extreme cells where an offset would step outside. A
  companion arm compares exact in-range whole coordinates represented as JSON
  integers and floats (so `51` and `51.0` are exercised as equal). Method: one
  property test whose body iterates over each key in `ROUNDED_COORDINATE_KEYS`
  and two shapes (top level; inside an object inside an array, the `waypoints`
  shape). Artefact: same file, `coordinates_in_one_grid_cell_share_a_key`.
  Non-vacuity: offsets include nonzero values of both signs; manual negative
  control NC-2 (change `COORDINATE_PRECISION_FACTOR` to `1_000_000.0`) must
  fail this property. Before Stage C2, a D-2 witness (two distinct finite
  values above 1.8e303 under `lat`) must be shown failing as a promoted example
  — the red evidence for D-2.
- Obligation V-3 (coordinate divergence): payloads identical except for
  coordinate leaves in different grid cells yield different keys; and two
  distinct JSON integers under a coordinate key either yield different keys or
  are rejected (never silently collide). Method: property test; one generator
  draws `cell` in `-18_000_000..=18_000_000` and a nonzero `delta` in
  `-36_000_000..=-1` or `1..=36_000_000`. It clamps `cell + delta` into the
  admitted range; if that yields `cell` again, it selects the adjacent in-range
  cell (`cell + 1`, except at the maximum where it selects `cell - 1`). Thus
  every generated pair is distinct without filtering. A second arm draws
  arbitrary distinct `i64`/`u64` pairs. Artefact: same file,
  `coordinates_in_distinct_grid_cells_diverge`. Non-vacuity: rests on LEM-1's
  corollary, AXM-1, and AXM-4; the mutation gate must kill "`round_coordinate`
  returns a constant" mutants. The integer arm is the red evidence for D-3
  before Stage C2.
- Obligation V-4 (normalization idempotence): for every generated payload
  where `normalize(v)` succeeds, `normalize(normalize(v))` succeeds and equals
  `normalize(v)`, and — mandatorily, because `Value::eq` treats `-0.0 == 0.0`
  while the two serialize differently —
  `for_route_request(v) == for_route_request(&normalize(v))`. Method: property
  test over a general recursive JSON strategy (`prop_recursive`, depth ≤ 4, ≤ 8
  collection items, leaves covering null, bool, the **full** `i64` and `u64`
  ranges, the full finite `f64` range, and strings) whose object-key strategy
  is biased to emit the special key names (`themes`, `lat`, `lng`, …) with high
  probability. Float leaves mix `any::<f64>()` restricted to finite values by
  construction (`prop::num::f64::NORMAL | SUBNORMAL | ZERO`) with a
  sub-strategy concentrated in ±1,000 so both in-range and out-of-range
  coordinates are common. Artefact: same file, `normalization_is_idempotent`.
  Non-vacuity: the statistics guard proves the strategy reaches the sort and
  round branches and out-of-range coordinates; manual negative control NC-4
  (append a constant to every string during normalization) must fail
  idempotence. Before Stage C2 this property is expected to fail with a D-1
  counter-example — its shrunk input is the red evidence for D-1.
- Obligation V-5 (derivation totality and key format): for every generated
  payload, derivation never panics and returns either `Ok(key)` whose string
  matches `^route:v1:[0-9a-f]{64}$` (checked with explicit character-class
  assertions; no regex crate), or `Err(CoordinateOutOfRange { .. })`; and it
  returns `Ok` exactly when every coordinate leaf has `|value| <= 180` (the
  oracle is a small test-side walk that only *finds* coordinate leaves — it
  does not re-implement rounding). Note `RouteCacheKey::new` validates only
  emptiness and surrounding whitespace; hex-digest validation lives solely in
  `for_route_request` via `is_lowercase_hex_digest`, so the property's own
  assertions carry the format obligation. Method: property test over the same
  recursive strategy as V-4. Artefact: same file,
  `derived_keys_match_route_v1_format`. Non-vacuity: the mutation gate must
  kill mutants of `is_lowercase_hex_digest`, of the namespace formatting, and
  of the magnitude comparison.
- Obligation V-6 (order is material where canonicalization is not claimed):
  permuting an array of ≥ 2 pairwise-distinct elements with a non-identity
  permutation changes the key, in two variants: (a) all-string arrays under a
  key not in `SORTED_ARRAY_KEYS`; (b) arrays under a theme key containing at
  least one non-string element (the all-string guard leaves them unsorted).
  Distinctness is by construction (indexed strings `item-0`, `item-1`, …, plus
  one distinct non-string element for variant (b)); the permutation is rotation
  by one of a vector of length ≥ 2, which on pairwise-distinct elements is
  never the identity (a periodic array such as `[1, 1]` rotates to itself and
  would fail spuriously). Method: property tests. Artefacts: same file,
  `non_theme_array_order_is_material` and
  `non_string_theme_array_order_is_material`. Non-vacuity: the mutation gate
  must kill "sort every array unconditionally" mutants (e.g.
  `should_sort_array -> true`, all-string guard removed).
- Obligation V-7 (negative zero collapse): any coordinate value that rounds
  to zero (offsets within the zero grid cell, both signs) produces the same key
  as literal `0.0`. Method: the `cell == 0` class of V-2's generator, plus an
  explicit `rstest` example for the `json!(-0.0)` literal (proptest floats
  rarely emit signed zero). Artefact: V-2's property plus
  `cache_key/tests.rs::negative_zero_coordinate_collapses_to_zero`.
  Non-vacuity: the mutation gate must kill removal of the `rounded == 0.0`
  collapse.
- Obligation V-8 (single-leaf divergence — general injectivity guard):
  editing exactly one non-canonicalized leaf of a generated, successfully
  derived payload (changing a string value to a distinct string, flipping a
  boolean, or inserting a fresh key) changes the key. Its payload strategy
  keeps every coordinate number within ±180 so both derivations succeed,
  independently of the unrestricted payload strategy used for Stage C2
  rejection properties. Without this, a normalizer that dropped or rewrote
  non-special fields would pass V-1, V-4, V-5, and V-6. Method: property test;
  the strategy picks a payload and one edit whose changed-ness is guaranteed by
  construction (appending a suffix to a string; inserting under a key name the
  generator never otherwise emits). Artefact: same file,
  `edited_leaves_produce_distinct_keys`. Non-vacuity: rests on AXM-1/AXM-4; the
  mutation gate must kill identity-erasing mutants (e.g. the object branch
  returning `Value::Null`).
- Obligation V-9 (digest algorithm known-answer): one fixed, in-range
  payload maps to a precomputed `route:v1:<expected 64-hex SHA-256>` string,
  computed once from the canonical bytes with an independent tool (`sha256sum`)
  **before any Stage C2 fix**, and pinned as a literal. It is the only test
  that fails if the hash algorithm or the pre-hash serialization is swapped,
  and it doubles as the guard that fixes do not change in-range keys. The
  digest is not re-derived in test code; the comment beside the constant
  records the exact command that produced it. Method: example test (`rstest`).
  Artefact: `cache_key/tests.rs::route_request_key_matches_known_sha256_digest`.
  Non-vacuity: the mutation gate's `hash_route_request_value` mutants must be
  killed by this test.
- Obligation V-10 (coordinate admission bound): a payload containing any
  coordinate leaf with `|value| > 180` (any finite `f64`, any `i64`/`u64`, at
  any depth including inside arrays of objects) fails derivation with
  `CoordinateOutOfRange` naming that key and the number's textual form; values
  of exactly ±180 are admitted. When several leaves are out of range, which one
  is reported is deterministic (the first in canonical key order), and the
  property asserts only that the reported leaf is one of the offending ones.
  Method: property test plus an `rstest` boundary table (`180`, `-180`, `180.0`,
  `180.000001`, `-180.000001`, `i64::MAX`, `u64::MAX`, `1e308`, subnormal)
  with an `insta` snapshot of the error's `Display` text, matching the module's
  existing error-message snapshots. Artefacts: same file,
  `out_of_range_coordinates_are_rejected`;
  `cache_key/tests.rs::coordinate_bound_is_inclusive`. Non-vacuity: the
  mutation gate must kill `<=`→`<` and `>`→`>=` mutants of the bound comparison
  (the inclusive-boundary cases exist for this).

Standing non-vacuity mechanisms (in the committed suite, not one-off):

- Statistics guard `route_payload_reaches_special_keys`: draws 512 values
  from `route_payload()` on a fixed seed and asserts at least one payload
  contains (i) a theme key with an all-string array of length ≥ 2, (ii) an
  in-range coordinate leaf with a fractional value, and (iii) an out-of-range
  coordinate leaf. The "at least one" thresholds are deliberately slack so
  proptest version bumps or strategy edits do not flip them; the doc comment
  states that a failure after such a change means re-verifying witness reach
  and re-seeding, not loosening properties. Implementation sketch (the
  supported proptest 1.x sampling API):

  ```rust
  let mut runner = proptest::test_runner::TestRunner::deterministic();
  let strategy = route_payload();
  let mut reach = WitnessReach::default();
  for _ in 0..512 {
      let value = strategy
          .new_tree(&mut runner)
          .expect("strategy should produce a tree")
          .current();
      reach.record(&value);
  }
  assert!(reach.saw_sortable_themes);
  assert!(reach.saw_fractional_in_range_coordinate);
  assert!(reach.saw_out_of_range_coordinate);
  ```

- Mutation gate MUT-1: a scoped run of
  `cargo mutants --file backend/src/domain/ports/cache_key.rs -- --lib`
  executed after Stage C2 and again at EP-M4. The nightly mutation-testing
  workflow keeps this evidence standing thereafter. cargo-mutants generates the
  comparator swaps, guard-function `-> true`/`-> false` replacements,
  `round_coordinate -> constant`, relational-operator flips, and function-body
  erasures; it cannot commit a mutation and produces machine-readable
  transcripts.

- Manual negative controls (only what mutants cannot generate): NC-2
  (constant change to `COORDINATE_PRECISION_FACTOR`) and NC-4 (string-append
  inside normalization). Procedure: apply the mutation, run the focused
  property or harness and record the failure into `Artefacts and notes`
  including the filtered-in test count (a transcript showing zero filtered-in
  tests is vacuous and must be treated as a failed control). Before applying a
  control, record the intended worktree and index diffs for the file. Apply and
  reverse only the narrow temporary edit: restore the precision constant for
  NC-2, and remove only the injected string-append for NC-4. Never use
  whole-file checkout/restore or reverse the complete file diff, since that can
  discard the uncommitted Stage C2 fix. After reversal, confirm the temporary
  mutation is absent, the recorded Stage C2 diff remains in the worktree/index,
  and `git diff --check` passes before rerunning the focused property to green.
  The property module's doc comment points at this ExecPlan as the record of
  the negative-control evidence.

Deliberately not verified, with rationale:

- Object key-order invariance: vacuous in this build (AXM-2, BTreeMap); it
  cannot fail whatever the implementation does. Recorded here and in the module
  documentation instead of writing a tautological property. If `preserve_order`
  is ever enabled, this decision must be revisited — a note to that effect goes
  into the property module's doc comment.
- Geographic validity (latitude within ±90, longitude within ±180 per axis):
  request validation belongs to the route-request inbound boundary, not the
  cache key. The ±180 admission bound is a numeric-soundness bound applied
  uniformly to every coordinate key; it is what LEM-1 needs, no more.
- Non-number values under coordinate keys (for example `"lat": "51.5"`) and
  Unicode normalization or case-folding of theme strings: the canonical form
  has never claimed to normalize these; they pass through verbatim, so
  textually different values legitimately produce different keys. V-8 covers
  that they are not dropped.
- SHA-256 collision resistance and ryu formatting internals: axioms AXM-1
  and AXM-4. V-9 pins the *choice* of algorithm and serialization at the
  boundary, which is repository-owned.

## Plan of work

Stage A (reconnaissance plus Kani feasibility spikes): confirm the recon
findings still hold (`leta show normalize_route_request_value`,
`leta show round_coordinate`; `cargo tree -p backend -i serde_json`
re-confirming no `preserve_order`; `rg for_route_request backend/src`
re-confirming no production caller). Record the pre-split baseline test count
from a fresh `cargo test -p backend --lib cache_key` run (expected eighteen
cases; record the actual number in `Artefacts and notes`). The wrapper and
scalar-kernel Kani spikes both exceeded 30 minutes without a verdict. Record
their transcripts, then proceed with the documented hand-proof and dense-slice
fallback; do not add a Kani harness or claim a machine proof.

Stage B (mechanical split, first plateau):

1. In `backend/src/domain/ports/cache_key.rs`, replace
   `#[cfg(test)] mod tests { ... }` (lines 192–377) with
   `#[cfg(test)] mod tests;`. The TODO comment moves with the module and is
   deleted only in Stage E.
2. Create `backend/src/domain/ports/cache_key/tests.rs` containing the
   existing example tests verbatim (`super::` paths are unchanged for a child
   file module).

Stage C1 (properties that hold today; commit green): add
`backend/src/domain/ports/cache_key/tests/properties.rs`, declared via
`mod properties;` in `tests.rs`. Keep the module root small and split
strategies and invariant groups into themed children under `tests/properties/`
so no file approaches the repository's 400-line limit.

1. Strategy helpers in `tests/properties/strategies.rs`: `theme_array()`
   (small-alphabet string vectors), `grid_cell_pair()` (integer cell plus one
   nonzero offset magnitude used with both signs, below 0.49),
   `divergent_cells()` (cell plus nonzero delta), `admitted_route_payload()`
   for V-8 (recursive JSON with coordinate leaves bounded to the admitted
   domain), `payload_pair_with_single_leaf_edit()` for V-8 (vary one generated
   string leaf under `generated.payload.content`, holding the sibling edits
   fixed), and `rotate_by_one` for V-6. Add the unrestricted
   `coordinate_number()` and `route_payload()` strategies in Stage C2 for its
   full-domain properties. Free functions returning `impl Strategy<Value = T>`,
   matching the `generate_route/tests.rs` house style; no `prop_compose!` (no
   repository precedent). Property names deliberately omit the
   `route_request_key_` prefix used by the example tests — the `properties`
   module path disambiguates.
2. Compute and pin the V-9 known-answer constant now, against the
   unmodified production code, and add the test plus the V-7 `-0.0` example to
   `tests.rs`.
3. Add the properties that hold on today's code: V-1, V-6 (both variants),
   V-7's generator class inside V-2, and V-8. V-2 and V-3 are added with their
   in-range generators only at this stage.
4. Commit (EP-M2).

Stage C2 (counter-example-driven fixes; red then green in one commit):

1. Red: add full-domain `coordinate_number()` and `route_payload()` strategies,
   then add V-4 and V-5 over that payload strategy, the D-2 witness example,
   and the integer arm of V-3. Run the focused suite and record each shrunk
   counter-example (expected: D-1 from V-4, D-2 from the witness, D-3 from
   V-3's integer arm) in `Artefacts and notes`. Any counter-example not in
   D-1..D-3 is added to `Surprises & Discoveries` and handled under the same
   discipline.
2. Promote each shrunk counter-example to a named `rstest` case in
   `tests.rs` (these, too, fail at this point).
3. Green: in `cache_key.rs`, add `MAX_COORDINATE_MAGNITUDE: f64 = 180.0`;
   make `round_coordinate` return
   `Result<Number, RouteCacheKeyDerivationError>`, rejecting
   `|as_f64()| > MAX_COORDINATE_MAGNITUDE` with `CoordinateOutOfRange` (the
   check is exact for integers too: every integer the lossy cast distorts is
   far above 180); thread the `Result` through `normalize_route_request_value`
   and `hash_route_request_value` with `?`; drop the now-unreachable `from_f64`
   fallback in favour of an `expect` whose message cites LEM-1, or keep a typed
   fallback if the workspace's `missing_panics_doc`/`expect_used` lints make
   that cleaner (record which in `Decision Log`). Add the
   `CoordinateOutOfRange` variant with a `#[error(...)]` message naming key and
   value, and extend the `for_route_request` rustdoc with an `# Errors` section.
4. Add V-10 (property, boundary table, snapshot) and the statistics guard.
5. Re-run the focused suite: all green, V-9 and every pre-existing example
   test unchanged. Prune any regression seeds whose expected outcome the fix
   changed; commit the rest.
6. Run NC-2, NC-4, and MUT-1; record results. A property whose
   corresponding mutants survive is vacuous and must be rewritten before
   proceeding.
7. Pre-commit guard: both
   `git diff --cached -- backend/src/domain/ports/cache_key.rs` and
   `git diff -- backend/src/domain/ports/cache_key.rs` must show only the
   planned fix (no control residue); `git status --porcelain` inspected for
   `proptest-regressions/` files. Commit test and fix together (EP-M3).

Stage D (LEM-1 fallback evidence):

1. Do not add `cfg(kani)`, a proof module, or a passing-proof claim. Record the
   two bounded Kani spike transcripts and their resource measurements.
2. Add a deterministic test that visits every representable `f64` in 100,000
   adjacent bit patterns inside each of the `-180.0` and `180.0` boundaries.
   Assert the rounded result remains admitted, maps back to the original
   integer grid cell, and is bit-for-bit idempotent.
3. Keep the full-domain `proptest` properties and LEM-1 hand argument. State
   plainly that the boundary test is finite regression evidence and does not
   replace a machine proof over the admitted domain.
4. Document the Kani limitation and this verification choice in
   `docs/developers-guide.md`, then commit the fallback test and disposition in
   the EP-M3 plateau.

Stage E (documentation and closure):

1. Delete the TODO lines from `cache_key/tests.rs` module docs; extend the
   property module's doc comment with the documented exclusions (key-order
   vacuity under BTreeMap; geographic validity out of scope) and the pointer to
   this ExecPlan for the negative-control record.
2. `docs/backend-roadmap.md`: insert under §5.1, after 5.1.4:
   `- [ ] 5.1.4a. Add property-based tests for cache key canonicalization`
   `invariants (theme permutation invariance, coordinate rounding`
   `equivalence and divergence, normalization idempotence, single-leaf`
   `divergence, key format, known-answer digest, coordinate admission`
   `bound).` It is ticked to `[x]` only when this plan reaches COMPLETE.
3. `docs/wildside-backend-architecture.md` route-caching section: record
   ARCH-CACHEKEY-COORD-DOMAIN — coordinate-keyed numbers above 180 in magnitude
   are rejected with `CoordinateOutOfRange` because rounding is only a
   projection (LEM-1) inside that bound — and note that the contract is
   supported by the hand proof, example-based, property-based, dense-boundary,
   and mutation-tested coverage under `backend/src/domain/ports/cache_key/`.
   Also list the new error variant where the domain-model section describes
   `RouteCacheKey`.
4. `docs/developers-guide.md`: (a) add a property-testing bullet to the
   "Testing strategy" list at the top; (b) a short subsection under testing
   conventions recording: property modules live in `tests/properties.rs` child
   modules; strategies are free functions composing valid values (no structural
   filtering, and no narrowing a generator to dodge a counter-example); bodies
   use `prop_assert*`; `proptest-regressions/` files are committed and pruned
   when a property's expected outcome changes; shrunk failures are promoted to
   named `rstest` cases; scoped `cargo mutants --file` runs are the standing
   non-vacuity check; and this plan's Kani limitation and fallback are recorded
   with the testing guidance.
5. `docs/users-guide.md`: no change — no user-visible behaviour change (no
   production path calls the derivation yet); recorded in `Decision Log`.

Test-framework applicability, per the repository brief: `rstest` carries the
example-shaped checks (statistics-guard harness, `-0.0` literal, known-answer
digest, boundary table, promoted counter-examples); `proptest` carries the
payload invariants; LEM-1 is supported by its hand proof, generated properties,
and dense boundary-slice regression, while Kani was evaluated and declined
because it produced no verdict and its installed guidance excludes floating-
point precision; `insta` snapshots the new error message (consistent with the
module's existing error snapshots); `googletest` is not a dependency of this
crate (see `Decision Log`); no new `rstest-bdd` scenario is added because no
externally observable workflow changes (the existing Redis BDD scenario covers
the end-to-end contract with in-range coordinates, which this plan leaves
byte-identical); `verus` is unsuitable for the float lemma, for the reasons
given in `Verification plan`.

## Milestones and plateaus

- EP-M1 (mechanical test-module split). Outcome: `cache_key.rs` shrinks to
  ~193 production lines plus `#[cfg(test)] mod tests;`; all existing tests pass
  unchanged. Acceptance evidence: `cargo test -p backend --lib cache_key`
  passes with exactly the baseline count recorded in Stage A; `make check-fmt`
  and `make lint` show no new findings. Conformance check: no public interface,
  dependency, or format change. Recovery: single mechanical commit; revert
  restores the inline module.
- EP-M2 (properties that hold today). Outcome: V-1, V-2/V-3 (in-range
  arms), V-6, V-8 properties, V-7 and V-9 examples, all green against
  unmodified production code. Acceptance evidence: the focused property run
  reports the expected test count (assert via `grep 'test result: ok'` on the
  tee'd log under `set -o pipefail`; zero matched tests is a failure).
  Recovery: additive; revert restores EP-M1.
- EP-M3 (counter-examples fixed and LEM-1 fallback evidence). Outcome: red
  transcripts for D-1..D-3 recorded; `CoordinateOutOfRange` fix landed with
  V-4, V-5, V-10, full-domain V-3, promoted regression cases, and the
  statistics guard; V-9 unchanged; NC-2/NC-4 and MUT-1 recorded; dense
  boundary-slice regression and documented Kani disposition recorded.
  Acceptance evidence: focused suite green; MUT-1 kill list with no survivors
  in the named functions; boundary test passes; Kani spike transcripts
  explicitly show no proof verdict. Conformance check: in-range keys
  byte-identical (V-9 and pre-existing examples green without edits); the only
  public change is the new variant. Compatibility decision: the new variant is
  an additive public change with no consumers; no deprecation needed. Recovery:
  revert the fix and fallback-test commits to return to EP-M2.
- EP-M4 (documentation, roadmap, TODO removal). Outcome: TODO gone, roadmap
  item 5.1.4a present and ticked, architecture and developers' guides updated,
  second MUT-1 run recorded, all gates green. Acceptance evidence:
  `make check-fmt`, `make lint`, `make test` logs show no new failures (compare
  the lint log against a baseline log captured from `main` — the local Whitaker
  0.2.7 vs CI 0.2.6 skew produces known noise);
  `git grep "TODO: Add property-based tests" backend/src/domain/ports` returns
  nothing. Recovery: docs-only commits, trivially revertable.

## Concrete steps

All commands run from the repository root with `set -o pipefail` active in the
shell (the `| tee` pipes otherwise mask non-zero exits). Long outputs go through
`tee` to
`/tmp/$ACTION-wildside-backend-5-1-4a-cache-key-canonicalization-property-tests.out`
(abbreviated below as `b514a`). Gates run sequentially, never in parallel;
full-gate runs are delegated to the `scrutineer` subagent where possible.

```bash
set -o pipefail

# Stage A
cargo tree -p backend -i serde_json | head -20   # expect no indexmap parent
rg -n for_route_request backend/src               # expect only cache_key.rs
cargo test -p backend --lib cache_key 2>&1 | tee /tmp/test-wildside-b514a-baseline.out
grep 'test result:' /tmp/test-wildside-b514a-baseline.out   # record count (expected 18)
# Kani spikes (both timed out before a verdict; no harness is retained)
# Wrapper transcript: /tmp/kani-wildside-b514a-spike-ldpath.out
# Scalar-kernel transcript: /tmp/kani-wildside-b514a-kernel-spike.out

# Stage B
cargo test -p backend --lib cache_key 2>&1 | tee /tmp/test-wildside-b514a-m1.out
grep 'test result:' /tmp/test-wildside-b514a-m1.out   # identical count to baseline

# Stage C1 / C2 focused loop
cargo test -p backend --lib cache_key 2>&1 | tee /tmp/test-wildside-b514a-m2.out
grep 'test result:' /tmp/test-wildside-b514a-m2.out   # 0 matched tests is a FAILURE
# V-9 constant: serialize the normalized fixed payload to a file from a
# scratch test, then:
sha256sum /tmp/b514a-v9-canonical.json

# Stage C2 mutation gate (also rerun at EP-M4)
cargo mutants --file backend/src/domain/ports/cache_key.rs -- --lib 2>&1 \
  | tee /tmp/mutants-wildside-b514a.out
# expect caught mutants for normalize_route_request_value, round_coordinate,
# should_sort_array/should_round_coordinate, is_lowercase_hex_digest,
# hash_route_request_value; record survivors

# Stage C2 pre-commit guards
git diff -- backend/src/domain/ports/cache_key.rs   # only the planned fix
git status --porcelain                              # inspect proptest-regressions

# Stage D
# Run dense_rounding_slice_near_coordinate_bound with the focused suite.

# Stage E / gates (delegate to scrutineer; sequential)
make check-fmt 2>&1 | tee /tmp/check-fmt-wildside-b514a.out
make lint      2>&1 | tee /tmp/lint-wildside-b514a.out
make test      2>&1 | tee /tmp/test-wildside-b514a.out
```

Note: `make check-fmt` and `make lint` are known to exit 0 even when sub-checks
fail; read the tee'd logs, not the exit codes. `make lint` may show
pre-existing Whitaker suite findings caused by a local 0.2.7 vs CI 0.2.6
version skew; diff against a `main` baseline log so only newly introduced
findings block progress.

Commit after each stage (B, C1, C2, D, each Stage E document), using file-based
commit messages per the `commit-message` skill. The C2 commit message must cite
the red transcripts recorded in this document.

## Validation and acceptance

Acceptance is behavioural:

1. Before Stage C1, `cargo test -p backend --lib cache_key::tests::properties`
   reports zero matching tests (module absent). After Stage C2 it reports the
   nine named properties plus the statistics guard, all passing, and
   `cache_key::tests` additionally reports the `-0.0`, known-answer,
   boundary-table, and promoted counter-example cases.
2. Red evidence for each fix: the Stage C2 transcripts show V-4 failing
   with a D-1 input (expected shape below), the D-2 witness failing, and V-3's
   integer arm failing with a D-3 pair, each with a nonzero filtered-in test
   count; after the fix the same tests pass and the promoted cases assert
   `CoordinateOutOfRange`.

   ```plaintext
   Test failed: assertion failed: `(left == right)` ...
   minimal failing input: value = {"lat": <shrunk magnitude above ~2e10>}
   test result: FAILED. 0 passed; 1 failed
   ```

3. Non-vacuity evidence: the MUT-1 transcript shows the expected mutant
   kills; NC-2 and NC-4 fail their target properties while applied and pass
   after a verified-clean revert; the dense boundary-slice test covers every
   representable value in its documented ULP intervals.
4. In-range stability: V-9 and every pre-existing example test pass without
   edits across the fix commit.
5. `make check-fmt`, `make lint`, and `make test` logs show no findings
   beyond the `main` baseline.
6. `docs/backend-roadmap.md` contains the ticked 5.1.4a entry; the TODO
   comment is gone from the ports module; the architecture document records the
   coordinate admission bound; the developers' guide lists property testing in
   its testing-strategy overview.

## Idempotence and recovery

Every stage is an additive, mechanical, or single-fix commit; re-running any
command is safe. If a property fails intermittently, the committed
`proptest-regressions/` seed makes the failure reproducible (stored seeds
replay first). Prune regression seeds in the same commit as any fix that
changes their expected outcome. Rollback at any plateau is `git revert` of that
stage's commit; no data, schema, or wire format is touched (no cache is
deployed). Recover an interrupted negative control using the mutation-specific
procedure under `Verification plan`: reverse only its temporary edit, confirm
it is absent, and verify any Stage C2 diff remains. Never restore the whole
file. Temporary Kani harnesses were removed; their logs and no-proof
disposition are retained in this plan.

## Interfaces and dependencies

Public interface change (the only one):

```rust
pub enum RouteCacheKeyDerivationError {
    // existing: Hash(PayloadHashError), Validation(RouteCacheKeyValidationError)
    /// A coordinate-keyed number exceeds the admitted magnitude of 180.
    #[error("route request coordinate `{key}` is out of range: {value}")]
    CoordinateOutOfRange { key: String, value: String },
}
```

Private changes in `cache_key.rs`: `MAX_COORDINATE_MAGNITUDE: f64 = 180.0`;
`round_coordinate`, `normalize_route_request_value`, and
`hash_route_request_value` become fallible.

The test modules consume `RouteCacheKey`, `RouteCacheKeyDerivationError`,
`RouteCacheKeyValidationError`, and the private items
`normalize_route_request_value`, `round_coordinate`, `SORTED_ARRAY_KEYS`,
`ROUNDED_COORDINATE_KEYS`, `COORDINATE_PRECISION_FACTOR`, and
`MAX_COORDINATE_MAGNITUDE` via the `super::` chain (matching the
`apalis_route_queue` precedent).

Dependencies: dev-dependencies already present — `proptest = "1"`,
`rstest = "0.26"`, `insta`, `serde_json` (`json!` macro), `pretty_assertions`.
Developer tooling: `cargo-mutants` (also run nightly by
`.github/workflows/mutation-testing.yml`) and `cargo-kani` 0.67. Kani is not
wired into CI or used for LEM-1 because its installed guidance excludes
floating-point precision and both bounded feasibility runs timed out.

## Progress

- [x] Stage A: dependency and caller facts reconfirmed; wrapper and
  scalar-kernel Kani spikes reached CBMC but exceeded 30 minutes. No proof is
  claimed; the hand-proof and dense-slice fallback is recorded.
- [x] Stage A: capture the baseline focused test count (18 passed, 0 failed,
  766 filtered out).
- [x] EP-M1 / Stage B: split `cache_key.rs` tests into `cache_key/tests.rs`;
  extraction is formatted, source and docs gates passed, and final CodeRabbit
  review completed with no findings.
- [x] EP-M2 / Stage C1: themed property modules and strategies; V-9 pinned
  against unmodified code; focused suite has 26 passing tests; full repository
  gates passed and final CodeRabbit review reported no findings.
- [ ] EP-M3 / Stage C2: D-1..D-3 red transcripts recorded.
- [ ] EP-M3 / Stage C2: counter-examples promoted to `rstest` cases.
- [ ] EP-M3 / Stage C2: `CoordinateOutOfRange` fix; V-4, V-5, V-10,
  full-domain V-3, statistics guard green; V-9 unchanged.
- [ ] EP-M3: NC-2, NC-4 transcripts; MUT-1 kill list.
- [ ] EP-M3 / Stage D: dense boundary-slice test and written Kani disposition.
- [ ] EP-M4 / Stage E: TODO removal, roadmap 5.1.4a entry ticked,
  architecture doc (admission bound and variant), developers-guide bullet and
  subsection, second MUT-1 run, gates green.

## Surprises & discoveries

- Observation: the first Stage B gate pass exposed a rustfmt diff in the
  extracted test module, although `make check-fmt` continued to later format
  checks and returned success. `make fmt` has now formatted the module; Stage B
  must repeat `make check-fmt` on the clean result. The same pass found a
  pre-existing Pylint C1803 simplification in
  `tests/workflow_contracts/makefile_tooling_test.py`; the assertion was
  simplified without changing its meaning so the required repository lint gate
  can complete. Evidence is in the first gate logs under `/tmp`.
- Observation: the first Stage B CodeRabbit review found that the framework
  applicability summary still said Kani carried LEM-1 and the scalar rounding
  helper lacked documentation. The summary now records the hand proof,
  generated properties, and dense boundary-slice regression; the helper now
  documents five-decimal rounding and signed-zero canonicalization.
- Observation: the next review found that V-3's clamped delta could select the
  original cell again; its generator now substitutes an adjacent admitted cell
  in that case. An older revision note also still described the Kani harness as
  current, so it now records that Kani was declined and the
  hand-proof/generated-property/bounded-regression fallback applies. CodeRabbit
  also reported a missing test-module path while `cache_key/tests.rs` was
  untracked; the review patch view omitted that file. It was staged for
  subsequent reviews, and the comment did not recur.
- Observation: the fourth review found that the NC-2/NC-4 recovery instructions
  could discard a later uncommitted Stage C2 fix by checking out a whole file.
  The procedure now records the intended diffs and applies or reverses only the
  temporary mutation, then verifies that the mutation is gone and the Stage C2
  diff remains before rerunning the green property.
- Observation: final Stage B review found no remaining concerns after the
  recovery procedure and its general rollback cross-reference were corrected.
  The docs-only gates and `coderabbit review --agent` both passed on the
  complete four-file staged change.
- Observation: the first Stage C1 full gate pass exposed a Whitaker lint
  requirement for inner module documentation on every new Rust child module.
  Purpose-specific `//!` comments now describe the four property modules; all
  other deterministic gates passed before this lint correction.
- Observation: the first clean Stage C1 CodeRabbit review found that proptest's
  `NORMAL`, `SUBNORMAL`, and `ZERO` strategies default to positive values, and
  the uniform grid-cell range almost never draws zero. The float generator now
  combines each finite class with both sign flags, and the grid-cell generator
  has a dedicated zero arm for V-7.
- Observation: the next Stage C1 review found that V-8 must use payloads whose
  coordinate leaves are all admitted, so its intended leaf edit is reached
  after Stage C2 adds coordinate rejection. The new `admitted_route_payload()`
  composes the same recursive shape with bounded coordinate numbers;
  `route_payload()` remains full-domain for idempotence and rejection
  properties.
- Observation: after bounding V-8's coordinate leaves, the first Stage C1
  deterministic gate pass succeeded and CodeRabbit reported no findings.
  Follow-up reviews on the final staged patch then found stale test-module
  references, insufficiently nested V-8 edits, a guard that omitted staged
  diffs, and a missing Python assertion diagnostic. The plan, V-8 pair
  generator, guard, and diagnostic now address each finding. The final complete
  gate run and review are recorded below.
- Observation: the final staged CodeRabbit review found that this orientation
  section still described the former inline test module and pre-split source
  line count. It now points to `cache_key/tests.rs` and the property-module
  directory. Subsequent reviews refined V-8's generator: its request pair now
  changes one generated `field-*` string leaf under
  `generated.payload.content`, while keeping the random sample and both sibling
  edit fields fixed. The reviews also requested staged-diff coverage in the
  pre-commit guard and an invocation diagnostic in the workflow tooling test.
  Those corrections passed all deterministic gates and the final CodeRabbit
  review before the C1 commit.
- Observation: local Kani guidance conflicts with LEM-1's model assumption.
  Evidence: `kani/SKILL.md` says Kani does not model floating-point precision;
  the plan's AXM-5 assumes bit-precise floating-point semantics. Impact: the
  Stage A spike must demonstrate that the installed `cargo-kani` actually
  supports the operations and assertions used here before the proof stage can
  proceed. A successful run that treats operations imprecisely is not proof
  evidence.
- Observation: the first Kani spike stopped in a dependency build script
  because `kani-compiler` could not load its bundled LLVM shared library.
  Evidence: `/tmp/kani-wildside-b514a-spike.out` and
  `/tmp/kani-wildside-b514a-spike-retry.out`; both ended before harness
  compilation. `prover-tools kani install --version 0.67.0 --repo-root .`
  restored the bundled library, and direct execution of
  `kani-compiler --version` succeeds; exporting the Kani toolchain library path
  lets Cargo's build scripts reach CBMC. The wrapper harness then exceeded the
  30-minute feasibility limit: 189 VCCs, 15 remaining after simplification, two
  intermediate satisfiable results, then a stall in the third reduction at
  30m12s. Impact: no proof result is claimed; the next spike targets the pure
  scalar kernel.
- Observation: Kani's project-integration guidance requires either a harness
  inventory or a written disposition when a requested harness is declined.
  Impact: no unsupported harness will be committed; Stage E will document the
  measured tool limitation and the test-based fallback in the developers' guide.
- Observation: object key-order invariance — the headline "generated key
  ordering" clause of the original TODO — is untestable in this build. Evidence:
  `Cargo.lock` shows `serde_json` 1.0.150 without `indexmap`; `Value::Object`
  is `BTreeMap`-backed, so key order is normalized before the code under test
  runs. Impact: the plan replaces that clause with the idempotence property
  (V-4) and a documented exclusion.
- Observation: three coordinate-rounding defects exist at magnitudes no real
  coordinate occupies (D-1 idempotence failure above ~2^51 / 10^5; D-2 rounding
  skipped on multiply overflow above ~1.8e303; D-3 distinct integers beyond
  2^53 collapsing). Evidence: pinned `serde_json-1.0.150/src/number.rs` lines
  162–171; `round_coordinate` arithmetic at `cache_key.rs:173–182`; IEEE-754
  error analysis in LEM-1. Impact: first revision bounded generators to
  ±1,000,000 to avoid them. Revised (see `Decision Log`): because no production
  path uses the derivation, these are fixed at the source with an admission
  bound and proved correct inside it, and generators now span the full domain.
- Observation (design panel): no existing or previously planned test would
  fail if the hash algorithm or pre-hash serialization were swapped —
  `cache_key.rs` calls `Sha256::digest` directly and the example tests check
  only digest shape. Evidence: `hash_route_request_value` at
  `cache_key.rs:117–126`;
  `route_request_key_has_expected_namespace_and_hash_shape` checks length/hex
  only. Impact: obligation V-9 (known-answer test) added.
- Observation (design panel): the repository already runs nightly
  cargo-mutants over `backend/`, making hand-scripted mutation controls mostly
  redundant. Evidence: `.github/workflows/mutation-testing.yml`;
  `cargo-mutants` 27.x installed locally. Impact: manual controls reduced to
  the mutation shapes cargo-mutants does not generate (NC-2 and NC-4).
- Observation: `RouteCacheKey::for_route_request` has no production caller.
  Evidence: `rg for_route_request backend/src` matches only `cache_key.rs`; the
  BDD test is the sole external user. Impact: counter-examples are treated as
  authoritative defects; adding a public error variant carries no compatibility
  cost.

## Decision log

- Decision: treat counter-examples as authoritative; fix D-1..D-3 in
  production and let generators span the full finite number domain, rather than
  bounding generators to avoid them. Rationale: the commissioning engineer
  directed that counter-example findings carry higher authority because the
  production code is not yet in use; there are no deployed keys or callers to
  protect, so fencing off the defects would only defer them to the first real
  caller. Date/Author: 2026-09-28, planning agent, on commissioning-engineer
  instruction.
- Decision: fix by rejecting coordinate-keyed numbers with magnitude above
  180 (`CoordinateOutOfRange`), uniformly across all coordinate keys.
  Alternatives considered: (a) pass out-of-range values through unrounded —
  total and idempotent, but silently makes noise-sensitive keys and hides
  malformed requests; (b) clamp to ±180 — collapses distinct invalid requests
  onto one valid key, a cache-poisoning shape; (c) make rounding correct for
  every finite `f64` (identity once the `f64` spacing exceeds the grid,
  integer-exact handling for large integers) — feasible but adds
  magnitude-dependent branches for inputs that are never valid coordinates, and
  a larger proof obligation; (d) per-axis geographic bounds (±90 for latitude)
  — conflates cache-key soundness with request validation, which belongs at the
  inbound boundary. The uniform ±180 bound is the smallest rule under which
  LEM-1 holds with a wide margin and every in-range key is unchanged.
  Date/Author: 2026-09-28, planning agent.
- Decision: decline a Kani harness for LEM-1 in this change and support the
  claim with its hand proof, generated properties, and a dense boundary-slice
  regression. Rationale: the installed Kani guidance says floating-point
  precision is not modelled; the wrapper and isolated-kernel spikes each
  exceeded 30 minutes without a verdict (details in `Artefacts and notes`). A
  green result would not establish the requested IEEE-754 claim. This is the
  documented written disposition required by the Kani project-integration
  guidance; revisit with a verifier that explicitly supports these operations.
  Date/Author: 2026-09-28, implementation agent.
- Decision: isolate the existing scalar rounding expression in the private
  `round_coordinate_value` function so the dense boundary-slice test can call
  the exact operation without constructing `serde_json::Number` values.
  Repository search found no equivalent helper. The helper remains local to
  `cache_key.rs` and has no independent reuse contract or public interface.
  Date/Author: 2026-09-28, implementation agent.
- Decision: place Stage C1 property groups and shared strategies in themed
  child modules under `tests/properties/` from the outset. The strategy helpers
  and seven properties would make one file approach the repository's 400-line
  limit; child modules keep each responsibility reviewable and below that cap.
  Date/Author: 2026-09-28, implementation agent.
- Decision: verify payload-level invariants with `proptest`, plus `rstest`
  examples and a scoped `cargo-mutants` gate. Rationale: the payload domain is
  unbounded and recursive; property tests with mutation-gate non-vacuity
  evidence give proportionate rigour there. Date/Author: 2026-08-16, planning
  agent.
- Decision: adopt the design panel's amendments — V-8 (single-leaf
  divergence) and V-9 (known-answer digest); replace five manual negative
  controls with the MUT-1 mutation gate; executable pre-commit guards;
  `set -o pipefail` and scripted count assertions on tee'd pipes; AXM-4.
  Alternatives rejected: an equivalence-oracle reference canonicalizer (shares
  the implementation's mental model and cannot express divergence); a single
  perturbation-enum property (muddles failure attribution); an optional
  composed-perturbations property (deferred). Date/Author: 2026-08-16, planning
  agent, on panel review. The panel's ±1,000,000 generator bound is superseded
  by the 2026-09-28 decisions.
- Decision: do not add `googletest` assertions despite the general testing
  brief. Rationale: `googletest` is not a dependency of the `backend` crate;
  adding it violates the no-new-dependencies constraint, and proptest bodies
  must use `prop_assert*` to preserve shrinking anyway. Date/Author:
  2026-08-16, planning agent.
- Decision: one new `insta` snapshot (the `CoordinateOutOfRange` message)
  and no new `rstest-bdd` scenario. Rationale: the module already snapshots its
  error messages; no externally observable workflow changes, and the existing
  Redis-backed BDD scenario uses in-range coordinates whose keys are unchanged.
  Date/Author: 2026-09-28, planning agent.
- Decision: add a `5.1.4a` checkbox to `docs/backend-roadmap.md` §5.1 even
  though the roadmap has no lettered-item precedent. Rationale: the
  commissioning request explicitly assigned roadmap reference 5.1.4a and
  requires the entry to be markable as done on completion. Date/Author:
  2026-08-16, planning agent.
- Decision: `docs/users-guide.md` is not updated. Rationale: no production
  path calls the derivation, so no user can observe the new rejection; the
  architecture document records the rule for the future caller. Date/Author:
  2026-09-28, planning agent.
- Decision: record the coordinate admission bound in the architecture
  document rather than an ADR. Rationale: it is a local, reversible rule inside
  one private function with no consumers; the Decision Log entry and the
  architecture section suffice. Date/Author: 2026-09-28, planning agent.
- Decision: commit `proptest-regressions/` files if produced, promote shrunk
  failures to named `rstest` cases, and prune seeds whenever a fix changes
  their expected outcome. Rationale: proptest failure-persistence convention;
  stale seeds would keep CI red or silently stop testing the original
  counter-example. Date/Author: 2026-09-28, planning agent.
- Implementation constraint: the Kani skill requires written justification
  when a requested harness is declined. Stage E will document why this verifier
  cannot substantiate LEM-1 and the chosen test fallback. Source: local `kani`
  skill, `Project integration`.

## Outcomes & retrospective

To be completed as milestones land. Candidate follow-up to assess at close:
evaluate a verifier with explicit IEEE-754 support for LEM-1; no Kani harness
or CI wiring is planned for this change.

## Artefacts and notes

Stage A baseline, captured at `4bb8187860ddefcb74d3460b439952772bc011e4`:
`cargo test -p backend --lib cache_key` passed 18 tests, with 766 filtered out,
in 886 seconds. The full output is
`/tmp/test-wildside-backend-5-1-4a-cache-key-canonicalization-property-tests.out`.
The run waited for the shared Cargo package-cache and build-directory locks;
neither was bypassed.

Stage B validation: the final full gate pass succeeded: `make check-fmt`,
`make lint`, focused cache-key tests (18 passed, 766 filtered), backend
doctests (160 passed, 96 ignored), `make test` (1,480 nextest passed, 4
skipped; 1 trybuild and 107 Python tests passed), `make typecheck`,
`make markdownlint` (125 files), and `make nixie`. The final staged docs-only
revalidation also passed formatting, Markdown lint (125 files, no errors), and
Mermaid validation. CodeRabbit reviewed all four staged paths and reported no
findings. Gate logs are under `/tmp` with the branch suffix
`backend-5-1-4a-cache-key-canonicalization-property-tests`; the final review
log is
`/tmp/coderabbit-e1abce79-3a81-406b-8473-7b288e22b103-backend-5-1-4a-cache-key-canonicalization-property-tests-4.out`.

Stage C1 evidence: `cargo test -p backend --lib cache_key` passes 26 tests
(including the eight new C1 example/property tests), with 766 filtered out. The
full repository pass reported 1,488 Rust tests passed and four skipped, one
trybuild test passed, 40 workflow-contract tests passed, 90 frontend tests
passed, and 107 Python tests passed. Backend doctests passed 160 tests, with 96
ignored; formatting, lint, typecheck, Markdown lint (125 files), and Mermaid
validation also passed. Gate logs are under `/tmp` with the full branch slug
`e1abce79-3a81-406b-8473-7b288e22b103-backend-5-1-4a-cache-key-canonicalization-property-tests`;
the final CodeRabbit review found no concerns and is recorded at
`/tmp/coderabbit-e1abce79-3a81-406b-8473-7b288e22b103-backend-5-1-4a-cache-key-canonicalization-property-tests-12.out`.
V-9 pins
`route:v1:064a4c57f3b53c1461a025298f66a1393b7b3a0c19cfe19c5297c063c7d06be9`.
The digest was computed before Stage C2 with this independent command:

```sh
printf '%s' '{"origin":{"lat":51.5,"lng":-0.1},"preferences":{"interestThemeIds":["art","history"]}}' | sha256sum
```

Dependency confirmation: `cargo tree -p backend -e features -i serde_json` shows
`serde_json` 1.0.150 with `default`, `raw_value`, and `std`, without
`preserve_order`. Caller search (`rg -n for_route_request backend/src`) finds
only the implementation and its local tests; CodeGraph finds the same local
test callers and the behavioural test's `derive_equivalent_keys` step.

The wrapper-harness Kani spike reached CBMC with
`LD_LIBRARY_PATH=/home/leynos/.kani/kani-0.67.0/toolchain/lib`, then exceeded
30m12s without a verdict. Its transcript reports 189 VCCs, 15 after
simplification, two intermediate satisfiable results, and a third propositional
reduction still running at the cutoff. RSS was about 83 MiB and system memory
remained below the 16 GB limit. Log:
`/tmp/kani-wildside-b514a-spike-ldpath.out`.

The isolated scalar-kernel spike passed
`prover-tools kani check-version --repo-root . --expected-version 0.67.0`, then
exceeded 30m05s without a verdict. It reduced the model to 8 VCCs; two solver
reductions reported SAT, and the third remained active at the cutoff. RSS was
about 131 MiB; system memory remained below the limit. Log:
`/tmp/kani-wildside-b514a-kernel-spike.out`. These intermediate results are not
proof evidence. Both attempts were stopped by their own foreground command at
the plan's 30-minute limit.

Still to record: the V-9 constant and the command that produced it; the
D-1..D-3 red transcripts with shrunk inputs; the MUT-1 kill lists (Stage C2 and
EP-M4); the NC-2/NC-4 transcripts with their filtered-in counts; and the dense
boundary-slice test result.

______________________________________________________________________

Revision note (2026-09-28): revised on the commissioning engineer's instruction
that counter-example findings carry higher authority because the production
code is not yet in use. The plan is no longer test-only: the three
planning-time defects (D-1 idempotence failure, D-2 skipped rounding on
overflow, D-3 large-integer collapse) are now fixed at the source by a uniform
±180 coordinate admission bound with a new `CoordinateOutOfRange` error
variant, instead of being hidden by a ±1,000,000 generator bound. Generators
now span the full finite `f64` and `i64`/`u64` domains; V-4 and V-5 are
restated over that domain, V-3 gains an integer-collision arm, and V-10
(admission bound) is added. LEM-1 is supported by its hand proof, generated
properties, and the bounded boundary-slice regression; the proposed Kani
harness was declined after the feasibility spikes produced no verdict (see the
following revision note). Stages were restructured into C1 (properties green
today), C2 (red transcripts, then fix), D (bounded regression), and E (docs);
milestones gained EP-M4. Tolerances now permit counter-example fixes within
interface and canonical-form limits, and V-9 is pinned before any fix so it
guards in-range key stability.

Revision note (2026-09-28): Stage A's wrapper and isolated-kernel Kani spikes
both reached CBMC but exceeded 30 minutes without a verdict. The installed Kani
guidance also excludes floating-point precision. Stage D now declines a Kani
claim and instead requires LEM-1's hand derivation, the existing full-domain
generated properties, and an exhaustive check of a 100,000-ULP slice inside
each ±180 boundary. The developers' guide will state this limitation and
disposition explicitly.

Revision note (2026-09-28): post-gate CodeRabbit reviews identified that V-8
must vary a generated string leaf under `generated.payload.content`, keeping
the random sample and sibling fields constant; the pre-commit guard must
include staged changes; and the workflow contract test should report recorded
invocations on failure. The property, guard, assertion diagnostic, and module
references now reflect those findings. All deterministic gates passed on the
final staged C1 patch, and CodeRabbit reported no remaining concerns. C2's
scope is unchanged.

Revision note (2026-08-16): revised after the six-lens design panel review.
Added V-8 and V-9; corrected the `canonicalize_and_hash` and
`from_f64`-fallback claims; replaced five manual controls with the scoped
cargo-mutants gate; added AXM-4; specified distinctness-by-construction and
rotation-by-one for V-6; mechanized the pre-commit guards and pipefail
discipline; corrected the baseline test count; renamed two properties.
