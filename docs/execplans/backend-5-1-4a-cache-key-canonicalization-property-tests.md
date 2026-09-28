# Add property-based tests for route cache key canonicalization invariants

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
`Outcomes & Retrospective`, `Conformance Basis`, and `Verification Plan` must
be kept up to date as work proceeds.

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
rejecting out-of-range coordinates with a typed error, and proving the rounding
step is a projection on the admitted domain.

The observable outcome for a developer is: `cargo test -p backend cache_key`
runs the new property suite over the full finite JSON number domain; each
property demonstrably fails when the canonicalization logic is broken
(mutation-testing and seeded-fault evidence is recorded below); a Kani harness
proves rounding idempotence for every admitted `f64`; route requests whose
coordinate-keyed numbers exceed ±180 in magnitude fail key derivation with
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

- `backend/src/domain/ports/cache_key.rs` (377 lines) — the canonicalization
  seam. `RouteCacheKey::for_route_request(payload: &serde_json::Value)`
  normalizes the payload via the private `normalize_route_request_value`,
  hashes the compact serialization of the normalized value with
  `sha2::Sha256::digest` directly (in the private `hash_route_request_value`,
  lines 117–126; `crate::domain::idempotency::PayloadHash` is used only as a
  byte container via `from_bytes`/`to_hex` — `canonicalize_and_hash` from the
  idempotency module is **not** called on this path), and formats
  `route:v1:<hex digest>`. The inline `#[cfg(test)] mod tests` (lines 192–377)
  holds nine `rstest`/`insta` test functions, which expand to eighteen test
  cases under `rstest` parameterization.
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
  the mechanical test-module split; removal of the TODO comment; and fixes for
  counter-examples to the obligations in `Verification plan` (the known defects
  D-1..D-3 and any further counter-example found inside the generator domain).
  Every production fix must be preceded by a recorded failing property or
  example (red), then land together with that test passing (green) in one
  commit.
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
  dependencies. Declaring `cfg(kani)` via the workspace `unexpected_cfgs` lint's
  `check-cfg` list is permitted.
- Respect the repository's 400-line file limit (`AGENTS.md`): after the
  split, `cache_key.rs`, `cache_key/tests.rs`, the property module, and the
  proof module must each stay under 400 lines. Contingency: if
  `tests/properties.rs` approaches the limit, split it into a
  `tests/properties/` directory (`strategies.rs` for generators, sibling files
  per invariant group), following the `enrichment/tests/bounding_box.rs`
  themed-file precedent.
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
  not complete within 30 minutes or 16 GB of memory, stop the Kani stage,
  record the failure mode, and escalate with the options in `Risks` (the hand
  proof LEM-1 and the full-domain property remain in force).
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
- Risk: Kani cannot build or solve the harness in reasonable time (the
  `backend` crate has a large dependency graph, and bit-precise `f64`
  multiply/round/divide chains are expensive for CBMC). Severity: medium.
  Likelihood: medium. Mitigation: Stage A runs a throwaway spike first. Kani
  only generates code reachable from the harness, and `round_coordinate` is
  pure arithmetic, so the build is expected to be feasible; if not, the
  fallback options for escalation are (a) a unit-level check over every `f64`
  in a dense slice near the magnitude bound, backed by LEM-1's hand proof, or
  (b) moving the rounding arithmetic into a dependency-free module the harness
  can target.
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
  -> cache_key::proofs::round_coordinate_is_a_projection (LEM-1, Kani)
ARCH-CACHEKEY-OWNERSHIP -> EP-M1 -> module split keeps tests inside the
  domain port module (no adapter involvement)
```

## Verification plan

The work verifies existing invariants over generated input ranges and, because
the fixes introduce one new contractual rule (the coordinate admission bound)
and one lemma the rule exists to make true (rounding is a projection on the
admitted domain), adds an exhaustive machine check of that lemma.

Method selection:

- Property tests carry the payload-level invariants: the input domain
  (arbitrary JSON payloads, permutations, all finite numbers) is far too large
  to enumerate, and the invariants are relational (pairs of payloads), which
  proptest expresses directly.
- Kani carries LEM-1. The lemma quantifies over a single `f64`, a finite
  domain of at most 2^64 values, so bounded model checking with CBMC's
  bit-precise IEEE-754 semantics is an *exhaustive* proof, not a sample. Verus
  was considered and rejected for LEM-1: its support for reasoning about
  IEEE-754 floating-point arithmetic is too limited to state the rounding
  behaviour without axiomatizing exactly the facts being proved, which would
  make the proof a restatement. Kani was considered and rejected for
  `is_lowercase_hex_digest` (a two-line character-class check pinned by V-5 and
  the mutation gate; a harness would restate it) and for the payload-level
  properties (unbounded recursive structures).
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
- AXM-5: Rust `f64` arithmetic (`*`, `/`, `round`) is IEEE-754 binary64
  with round-to-nearest-even for `*` and `/`, and `round` rounds half away from
  zero; Kani/CBMC model these semantics bit-precisely.

Lemma LEM-1 (rounding is a projection on the admitted domain). Let
`MAX_COORDINATE_MAGNITUDE = 180.0` and `P = 100_000.0`. For every finite `f64`
`x` with `|x| <= 180`, define `n = round(x * P)` and
`r = canonical_zero(n / P)` (the current `round_coordinate` arithmetic). Then
(a) `|r| <= 180`, so `r` is itself admitted; (b) `round(r * P) == n`; and hence
(c) `round_coordinate(r) == r` bit-for-bit. Hand proof (recorded so the Kani
result has an independent argument): `P` is exactly representable.
`|x * P| <= 1.8e7 < 2^25`, so `n` is an integer with `|n| < 2^25`, exactly
representable. Division gives `n / P = (n / 10^5)(1 + d1)` with
`|d1| <= 2^-53`; multiplying back gives `(n / P) * P = n (1 + d1)(1 + d2)` with
`|d2| <= 2^-53`, so the error from `n` is at most
`|n| * (2^-52 + 2^-106) < 2^25 * 2^-51.9 < 2^-26`, far below the `1/2` needed
for `round` to return `n`; that is (b). Since `|n| <= 1.8e7` exactly (rounding
cannot overshoot the integer `1.8e7`), `|n / P| <= 180` after correctly rounded
division; that is (a). The zero case: `n == 0` gives `r == +0.0` by the
canonical-zero step, and `round(+0.0 * P) == 0`. (c) follows because `r` is a
function of `n` alone. A corollary used by V-3: `n` is recoverable from `r` by
(b), so distinct grid cells give distinct `r` and, by AXM-4, distinct bytes.
Method: Kani harness `round_coordinate_is_a_projection` asserting (a), (b), and
(c) for `kani::any::<f64>()` under
`kani::assume(x.is_finite() && x.abs() <= 180.0)`. Artefact:
`backend/src/domain/ports/cache_key/proofs.rs` (`#[cfg(kani)]`). Evidence:
`cargo kani -p backend --harness round_coordinate_is_a_projection` reporting
`VERIFICATION:- SUCCESSFUL`. Non-vacuity: the harness also runs with the bound
temporarily widened to `1e12` (manual control NC-5) and must report a
counter-example, proving the assumption is load-bearing and the assertions are
not trivially true; and a `kani::cover!` on `n != 0` confirms the non-zero
branch is reachable.

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
  full admitted range) and two offsets in ±0.49 of one grid unit, giving
  `(cell as f64 + offset) / 100_000.0`, clamped into ±180 only at the two
  extreme cells where an offset would step outside. Values are emitted both as
  floats and, when the offset is zero, as JSON integers where exact (so `51` and
  `51.0` are exercised as equal). Method: one property test whose body
  iterates over each key in `ROUNDED_COORDINATE_KEYS` and two shapes (top
  level; inside an object inside an array, the `waypoints` shape). Artefact:
  same file, `coordinates_in_one_grid_cell_share_a_key`. Non-vacuity: offsets
  include nonzero values of both signs; manual negative control NC-2 (change
  `COORDINATE_PRECISION_FACTOR` to `1_000_000.0`) must fail this property.
  Before Stage C2, a D-2 witness (two distinct finite values above 1.8e303 under
  `lat`) must be shown failing as a promoted example — the red evidence for
  D-2.
- Obligation V-3 (coordinate divergence): payloads identical except for
  coordinate leaves in different grid cells yield different keys; and two
  distinct JSON integers under a coordinate key either yield different keys or
  are rejected (never silently collide). Method: property test; one generator
  draws `cell` and a nonzero `delta: i64`, using `cell` and `cell + delta`
  clamped to the admitted range; a second arm draws arbitrary distinct `i64`/
  `u64` pairs. Artefact: same file,
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
  boolean, or inserting a fresh key) changes the key. Without this, a
  normalizer that dropped or rewrote non-special fields would pass V-1, V-4,
  V-5, and V-6. Method: property test; the strategy picks a payload and one
  edit whose changed-ness is guaranteed by construction (appending a suffix to
  a string; inserting under a key name the generator never otherwise emits).
  Artefact: same file, `edited_leaves_produce_distinct_keys`. Non-vacuity:
  rests on AXM-1/AXM-4; the mutation gate must kill identity-erasing mutants
  (e.g. the object branch returning `Value::Null`).
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
  (constant change to `COORDINATE_PRECISION_FACTOR`), NC-4 (string-append
  inside normalization), and NC-5 (widen the Kani assumption to `1e12`).
  Procedure: apply the mutation, run the focused property or harness and record
  the failure into `Artefacts and notes` including the filtered-in test count
  (a transcript showing zero filtered-in tests is vacuous and must be treated
  as a failed control), then revert with `git checkout -- <file>` and confirm
  `git diff --exit-code -- <file>` before re-running to green. The property
  module's doc comment points at this ExecPlan as the record of the
  negative-control evidence.

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

Stage A (no code changes, plus one throwaway spike): confirm the recon findings
still hold (`leta show normalize_route_request_value`,
`leta show round_coordinate`; `cargo tree -p backend -i serde_json`
re-confirming no `preserve_order`; `rg for_route_request backend/src`
re-confirming no production caller). Record the pre-split baseline test count
from a fresh `cargo test -p backend --lib cache_key` run (expected eighteen
cases; record the actual number in `Artefacts and notes`). Run the Kani spike:
a scratch `#[cfg(kani)]` harness calling today's `round_coordinate` with the
±180 assumption, run once, and discard with
`git checkout -- backend/src/domain/ports/cache_key.rs`. Record build time,
solve time, and verdict; this decides whether Stage D proceeds as planned or
escalates per `Tolerances`.

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
`mod properties;` in `tests.rs`.

1. Strategy helpers: `theme_array()` (small-alphabet string vectors),
   `grid_cell_pair()` (integer cell plus two in-cell offsets, cap ±0.49),
   `divergent_cells()` (cell plus nonzero delta), `coordinate_number()` (full
   finite `f64` plus full `i64`/`u64`, with an in-range-weighted arm),
   `route_payload()` (recursive JSON via `prop_recursive`, key names biased
   toward the special keys, coordinate leaves from `coordinate_number()`), a
   `rotate_by_one` helper for V-6, and a single-edit generator for V-8. Free
   functions returning `impl Strategy<Value = T>`, matching the
   `generate_route/tests.rs` house style; no `prop_compose!` (no repository
   precedent). Property names deliberately omit the `route_request_key_` prefix
   used by the example tests — the `properties` module path disambiguates.
2. Compute and pin the V-9 known-answer constant now, against the
   unmodified production code, and add the test plus the V-7 `-0.0` example to
   `tests.rs`.
3. Add the properties that hold on today's code: V-1, V-6 (both variants),
   V-7's generator class inside V-2, and V-8. V-2 and V-3 are added with their
   in-range generators only at this stage.
4. Commit (EP-M2).

Stage C2 (counter-example-driven fixes; red then green in one commit):

1. Red: add V-4 and V-5 over the full-domain `route_payload()`, the D-2
   witness example, and the integer arm of V-3. Run the focused suite and
   record each shrunk counter-example (expected: D-1 from V-4, D-2 from the
   witness, D-3 from V-3's integer arm) in `Artefacts and notes`. Any
   counter-example not in D-1..D-3 is added to `Surprises & Discoveries` and
   handled under the same discipline.
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
7. Pre-commit guard: `git diff -- backend/src/domain/ports/cache_key.rs`
   must show only the planned fix (no control residue);
   `git status --porcelain` inspected for `proptest-regressions/` files. Commit
   test and fix together (EP-M3).

Stage D (LEM-1 machine proof):

1. Declare `cfg(kani)` in the root `Cargo.toml` by adding an
   `unexpected_cfgs` entry with `check-cfg = ['cfg(kani)']` under
   `[workspace.lints.rust]` (matching the existing lint-level conventions
   there; use `deny` if that is the house level for the table).
2. Create `backend/src/domain/ports/cache_key/proofs.rs`, declared in
   `cache_key.rs` as `#[cfg(kani)] mod proofs;`, containing
   `round_coordinate_is_a_projection` per LEM-1, plus a `kani::cover!` for the
   non-zero branch.
3. Run the harness; run NC-5; record both transcripts.
4. Commit (part of EP-M3's plateau; separate commit).

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
   enforced by example-based, property-based, mutation-tested, and Kani-proved
   coverage under `backend/src/domain/ports/cache_key/`. Also list the new
   error variant where the domain-model section describes `RouteCacheKey`.
4. `docs/developers-guide.md`: (a) add a property-testing bullet to the
   "Testing strategy" list at the top; (b) a short subsection under testing
   conventions recording: property modules live in `tests/properties.rs` child
   modules; strategies are free functions composing valid values (no structural
   filtering, and no narrowing a generator to dodge a counter-example); bodies
   use `prop_assert*`; `proptest-regressions/` files are committed and pruned
   when a property's expected outcome changes; shrunk failures are promoted to
   named `rstest` cases; scoped `cargo mutants --file` runs are the standing
   non-vacuity check; and Kani harnesses live in `#[cfg(kani)] mod proofs;`
   child modules run with `cargo kani -p backend --harness <name>`.
5. `docs/users-guide.md`: no change — no user-visible behaviour change (no
   production path calls the derivation yet); recorded in `Decision Log`.

Test-framework applicability, per the repository brief: `rstest` carries the
example-shaped checks (statistics-guard harness, `-0.0` literal, known-answer
digest, boundary table, promoted counter-examples); `proptest` carries the
payload invariants; `kani` carries LEM-1; `insta` snapshots the new error
message (consistent with the module's existing error snapshots); `googletest`
is not a dependency of this crate (see `Decision Log`); no new `rstest-bdd`
scenario is added because no externally observable workflow changes (the
existing Redis BDD scenario covers the end-to-end contract with in-range
coordinates, which this plan leaves byte-identical); `verus` is unsuitable for
the float lemma, for the reasons given in `Verification plan`.

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
- EP-M3 (counter-examples fixed and LEM-1 proved). Outcome: red transcripts
  for D-1..D-3 recorded; `CoordinateOutOfRange` fix landed with V-4, V-5, V-10,
  full-domain V-3, promoted regression cases, and the statistics guard; V-9
  unchanged; NC-2/NC-4 and MUT-1 recorded; Kani harness
  `VERIFICATION:- SUCCESSFUL` and NC-5 counter-example recorded. Acceptance
  evidence: focused suite green; MUT-1 kill list with no survivors in the named
  functions; Kani transcripts. Conformance check: in-range keys byte-identical
  (V-9 and pre-existing examples green without edits); the only public change
  is the new variant. Compatibility decision: the new variant is an additive
  public change with no consumers; no deprecation needed. Recovery: revert the
  fix commit and the Kani commit to return to EP-M2.
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
# Kani spike (scratch harness, then discard)
cargo kani -p backend --harness round_coordinate_is_a_projection 2>&1 \
  | tee /tmp/kani-wildside-b514a-spike.out
git checkout -- backend/src/domain/ports/cache_key.rs
git diff --exit-code -- backend/src/domain/ports/cache_key.rs

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
cargo kani -p backend --harness round_coordinate_is_a_projection 2>&1 \
  | tee /tmp/kani-wildside-b514a.out
grep 'VERIFICATION:- SUCCESSFUL' /tmp/kani-wildside-b514a.out

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
   after a verified-clean revert; the Kani harness reports
   `VERIFICATION:- SUCCESSFUL` and NC-5 reports a counter-example.
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
deployed). Manual negative controls and the Kani spike are bracketed by
`git checkout -- <file>` plus `git diff --exit-code -- <file>`, so an
interrupted run is recovered by running those two commands.

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

The test and proof modules consume `RouteCacheKey`,
`RouteCacheKeyDerivationError`, `RouteCacheKeyValidationError`, and the private
items `normalize_route_request_value`, `round_coordinate`, `SORTED_ARRAY_KEYS`,
`ROUNDED_COORDINATE_KEYS`, `COORDINATE_PRECISION_FACTOR`, and
`MAX_COORDINATE_MAGNITUDE` via the `super::` chain (matching the
`apalis_route_queue` precedent).

Dependencies: dev-dependencies already present — `proptest = "1"`,
`rstest = "0.26"`, `insta`, `serde_json` (`json!` macro), `pretty_assertions`.
Developer tooling: `cargo-mutants` (also run nightly by
`.github/workflows/mutation-testing.yml`) and `cargo-kani` 0.67. Wiring Kani
into CI is out of scope for 5.1.4a and is recorded as a follow-up in
`Outcomes & Retrospective` if the harness proves stable.

## Progress

- [ ] Stage A: re-confirm recon facts; record baseline test count; Kani
  spike verdict recorded.
- [ ] EP-M1 / Stage B: split `cache_key.rs` tests into `cache_key/tests.rs`.
- [ ] EP-M2 / Stage C1: strategies; V-9 constant pinned against unmodified
  code; V-1, V-2/V-3 in-range, V-6, V-7, V-8 green.
- [ ] EP-M3 / Stage C2: D-1..D-3 red transcripts recorded.
- [ ] EP-M3 / Stage C2: counter-examples promoted to `rstest` cases.
- [ ] EP-M3 / Stage C2: `CoordinateOutOfRange` fix; V-4, V-5, V-10,
  full-domain V-3, statistics guard green; V-9 unchanged.
- [ ] EP-M3: NC-2, NC-4 transcripts; MUT-1 kill list.
- [ ] EP-M3 / Stage D: Kani harness successful; NC-5 counter-example.
- [ ] EP-M4 / Stage E: TODO removal, roadmap 5.1.4a entry ticked,
  architecture doc (admission bound and variant), developers-guide bullet and
  subsection, second MUT-1 run, gates green.

## Surprises & discoveries

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
  the mutation shapes cargo-mutants does not generate (NC-2, NC-4, NC-5).
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
- Decision: prove LEM-1 with Kani, not Verus.
  Rationale: LEM-1 is a statement about bit-precise IEEE-754 behaviour of a
  single `f64`; Kani/CBMC decide it exhaustively, whereas Verus would need the
  float facts axiomatized, restating the claim. A hand proof is recorded
  alongside as an independent argument. Date/Author: 2026-09-28, planning agent.
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

## Outcomes & retrospective

To be completed as milestones land. Candidate follow-up to assess at close:
wiring `cargo kani` into CI if the harness proves stable and fast.

## Artefacts and notes

To be populated during implementation: the Stage A baseline test count and Kani
spike verdict; the V-9 constant and the command that produced it; the D-1..D-3
red transcripts with shrunk inputs; the MUT-1 kill lists (Stage C2 and EP-M4);
the NC-2/NC-4/NC-5 transcripts with their filtered-in counts; the Kani success
transcript with solve time.

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
(admission bound) is added. The rounding step's projection property is stated
as lemma LEM-1 with a hand proof and an exhaustive Kani harness (Verus rejected
for float reasoning). Stages were restructured into C1 (properties green
today), C2 (red transcripts, then fix), D (Kani), and E (docs); milestones
gained EP-M4. Tolerances now permit counter-example fixes within interface and
canonical-form limits, and V-9 is pinned before any fix so it guards in-range
key stability.

Revision note (2026-08-16): revised after the six-lens design panel review.
Added V-8 and V-9; corrected the `canonicalize_and_hash` and
`from_f64`-fallback claims; replaced five manual controls with the scoped
cargo-mutants gate; added AXM-4; specified distinctness-by-construction and
rotation-by-one for V-6; mechanized the pre-commit guards and pipefail
discipline; corrected the baseline test count; renamed two properties.
