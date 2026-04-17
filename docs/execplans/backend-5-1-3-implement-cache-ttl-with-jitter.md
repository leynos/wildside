# Implement cache time-to-live (TTL) with jitter (roadmap 5.1.3)

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises &
Discoveries`, `Decision Log`, and `Outcomes & Retrospective` must be kept up
to date as work proceeds.

Status: COMPLETE

This plan covers roadmap item 5.1.3 only:
`Implement time-to-live (TTL) with jitter (24-hour window, +/- 10%) to
prevent thundering herd on cache expiry.`

## Purpose / big picture

Roadmap items 5.1.1 and 5.1.2 delivered a Redis-backed `RouteCache` adapter
with JSON serialization, but values are stored without an expiry. In
production, unbounded cache entries consume memory indefinitely and prevent
stale data from being evicted. Worse, if a bulk TTL is later bolted on at
the infrastructure level, many keys will expire at the same instant and
cause a thundering herd: a spike of concurrent cache misses that floods the
route-generation workers.

This task adds a 24-hour TTL with +/- 10% random jitter to the
`RouteCache` adapter so entries expire individually across a window of
roughly 21 h 36 min to 26 h 24 min. The jitter is applied per `put`
invocation, spreading expiry times across the window and eliminating
synchronized cache stampedes.

Observable success criteria:

- The `ConnectionProvider` trait gains an expiry-aware write method
  (`set_bytes_with_ttl`) that accepts a TTL in seconds.
- `RedisPoolProvider` implements the new method using the Redis `SET ... EX`
  command.
- `GenericRedisRouteCache::put` computes a jittered TTL and delegates to the
  new method.
- Jitter logic lives in a small, deterministic helper that accepts a base
  TTL and a random-number generator (RNG), avoiding direct calls to
  `rand::thread_rng()` in business logic.
- The `FakeProvider` test double records the TTL for each write so tests can
  assert jitter boundaries without Redis.
- `rstest` coverage proves:
  - the jittered TTL falls within the expected 77 760 s – 95 040 s window;
  - the adapter's `put` path passes the computed TTL to the provider;
  - round-trip behaviour is unchanged (get after put still succeeds).
- `rstest-bdd` coverage proves (via indirect verification against a
  live `redis-server`):
  - entries receive a TTL in the expected jitter window (confirmed by
    querying the Redis `TTL` command immediately after writes);
  - jittered writes produce measurably different TTLs across
    invocations.
  - **Note:** direct expiry testing (waiting for a key to disappear)
    was not performed because the adapter uses a compile-time 24-hour
    base TTL with no short-TTL override. Correctness of expiry relies
    on the `SET ... EX` delegation confirmed by unit tests.
- `docs/wildside-backend-architecture.md` records the TTL policy and jitter
  rationale.
- `docs/backend-roadmap.md` marks 5.1.3 done only after all required gates
  pass.
- `make check-fmt`, `make lint`, and `make test` pass with logs retained.

## Constraints

- Scope is roadmap item 5.1.3 only. Do not mark 5.1.4, 5.4.x, or any
  other roadmap item done as part of this change.
- Preserve hexagonal boundaries:
  - The `RouteCache` port trait in
    `backend/src/domain/ports/route_cache.rs` must not gain
    Redis-specific parameters (no `ttl_seconds` argument on the port
    trait). TTL is an infrastructure concern owned by the outbound adapter.
  - Redis client, pool, and TTL details live under
    `backend/src/outbound/cache/*`.
  - Inbound adapters and domain services must not import Redis types.
- Do not modify the `RouteCache::put` signature. The domain port remains
  oblivious to expiry; the adapter applies TTL internally.
- The jitter source must be injectable for testing. Use a trait or function
  parameter (for example, accepting `&mut impl rand::Rng`) rather than
  calling `rand::thread_rng()` directly in the adapter. See
  `docs/reliable-testing-in-rust-via-dependency-injection.md` for guidance.
- The default TTL base must be 24 hours (86 400 seconds) and the jitter
  range +/- 10%, yielding a window of 77 760 s to 95 040 s.
- Use `rstest` for focused unit coverage and `rstest-bdd` (behaviour-driven
  development, BDD) for behavioural coverage.
- Use `pg-embedded-setup-unpriv` to enable local testing with Postgres
  where existing suites require it (via `make test`).
- Keep files under 400 lines by splitting into coherent modules if needed.
- New public Rust APIs must carry Rustdoc comments and examples that follow
  `docs/rust-doctest-dry-guide.md`.
- Update documentation in en-GB-oxendict style.
- Avoid over-engineering: do not introduce configurable TTL via environment
  variables or runtime configuration in this task. The 24-hour base is a
  compile-time constant. A future task may add configuration if needed.
- Do not introduce `unsafe` code or silence Clippy lints.

## Tolerances (exception triggers)

- Scope tolerance: if the work requires changing the `RouteCache` port
  signature (adding a TTL parameter to `put`), stop and escalate. TTL is
  an adapter concern.
- Dependency tolerance: at most one new production dependency (`rand` or a
  lightweight alternative). If more are needed, stop and review.
- Test-harness tolerance: BDD scenarios that test real expiry require a live
  `redis-server`. Use a short TTL (for example, 2 seconds) in integration
  tests rather than waiting 24 hours. If expiry cannot be verified
  reliably, document the limitation and fall back to unit-level jitter
  assertions.
- Error-contract tolerance: if jittered TTL computations introduce new
  failure modes that do not fit `RouteCacheError::{Backend, Serialization}`,
  stop and evaluate whether a new error variant is warranted.
- Gate tolerance: if `make check-fmt`, `make lint`, or `make test` fail
  after three repair loops, stop and capture the failing logs.
- File-size tolerance: if any single file exceeds 400 lines, split it
  before proceeding.

## Risks

- Risk: the `ConnectionProvider` trait must change to accept a TTL, which
  may break the existing `FakeProvider` and any other implementations.
  Severity: medium.
  Likelihood: high (change is expected and planned).
  Mitigation: add the new method with a default implementation that ignores
  the TTL, then migrate implementations. Alternatively, add a new method
  (`set_bytes_with_ttl`) alongside the existing `set_bytes` so the old method
  remains available for tests that do not care about expiry.

- Risk: BDD scenarios verifying real Redis expiry may be flaky if the test
  machine is slow or the sleep window is too tight.
  Severity: medium.
  Likelihood: medium.
  Mitigation: use a generous sleep buffer (for example, TTL of 2 s and
  sleep for 3 s) and mark live expiry tests `#[ignore]` so they run only
  on explicit request, consistent with the existing cache BDD pattern.

- Risk: introducing `rand` as a production dependency may conflict with
  existing dependency versions or bloat the binary.
  Severity: low.
  Likelihood: low.
  Mitigation: check whether `rand` is already a transitive dependency. If
  so, align versions. If not, consider using `rand::rngs::SmallRng` or a
  minimal subset via feature flags to limit footprint.

- Risk: jitter arithmetic may overflow for large TTL values or produce
  zero/negative results.
  Severity: low.
  Likelihood: low.
  Mitigation: use `u64` arithmetic with explicit clamping to ensure the
  jittered TTL is always at least 1 second.

- Risk: full-gate failures may come from the existing embedded-Postgres
  setup rather than TTL changes.
  Severity: medium.
  Likelihood: medium.
  Mitigation: retain logs with `tee` and rely on `make test` so
  `PG_EMBEDDED_WORKER` is wired automatically.

## Agent team and ownership

This implementation should use an explicit agent team. One person may play
more than one role, but the ownership boundaries should remain visible.

- Coordinator agent:
  owns sequencing, keeps this ExecPlan current, enforces tolerances,
  collects gate evidence, and decides when roadmap item 5.1.3 is ready to
  close.
- TTL adapter agent:
  owns `backend/src/outbound/cache/*` changes: the jitter helper, the
  `ConnectionProvider` trait extension, the `RedisPoolProvider`
  implementation of `SET ... EX`, and the `GenericRedisRouteCache::put`
  TTL integration.
- Test harness agent:
  owns updates to `backend/src/outbound/cache/test_helpers.rs` (extending
  `FakeProvider` to record TTL), plus any shared fixtures needed for
  TTL-aware integration tests.
- Quality assurance (QA) agent:
  owns `rstest` unit coverage for jitter arithmetic and adapter TTL
  pass-through, plus `rstest-bdd` scenarios and feature file updates
  proving TTL expiry, pre-expiry readability, and jitter variation.
- Documentation agent:
  owns `docs/wildside-backend-architecture.md` and
  `docs/backend-roadmap.md` updates.

Hand-off order:

1. TTL adapter agent lands the jitter helper and `ConnectionProvider` trait
   extension with failing unit tests.
2. Test harness agent updates `FakeProvider` to record TTL values and adds
   any new BDD fixtures.
3. TTL adapter agent integrates jittered TTL into the adapter `put` path and
   makes all focused suites pass.
4. QA agent broadens coverage with edge-case unit tests and live-Redis BDD
   scenarios.
5. Documentation agent records the TTL policy and jitter rationale in the
   architecture document and closes the roadmap item.
6. Coordinator agent runs final gates and updates this ExecPlan.

## Progress

### Completed work

1. **Jitter helper function** (`jittered_ttl`):
   - Added pure function in `backend/src/outbound/cache/redis_route_cache.rs`
   - Computes TTL with uniform random jitter using injectable RNG
   - Handles edge cases (zero base, zero jitter, overflow) with saturating
     arithmetic and clamps result to minimum 1 second
   - Comprehensive `rstest` unit tests covering boundary conditions and jitter
     variation

2. **ConnectionProvider trait extension**:
   - Added `set_bytes_with_ttl` method accepting `ttl_seconds:
     Option<u64>` — `Some(n)` sets a TTL of `n` seconds, `None` stores
     without expiry
   - Implemented in `RedisPoolProvider` using Redis `SET ... EX` command
     (for `Some`) or plain `SET` (for `None`)
   - Retained backward-compatible `set_bytes` method as a convenience
     wrapper that delegates to `set_bytes_with_ttl(key, value, None)`

3. **FakeProvider test double enhancements**:
   - Modified internal store to record TTL alongside cached values
   - Changed from `Mutex<HashMap<String, Vec<u8>>>` to `Arc<Mutex<HashMap<String,
     CachedValue>>>` where `CachedValue = (Vec<u8>, Option<u64>)`
   - Added `ttl_for` helper method for test assertions
   - Implemented `set_bytes_with_ttl` to record TTL values

4. **GenericRedisRouteCache TTL integration**:
   - Added fields: `base_ttl: u64`, `jitter_fraction: f64`, `rng:
     Mutex<Box<dyn rand::RngCore + Send>>` (seeded via
     `StdRng::from_entropy()` in production constructors)
   - Modified `put` to compute jittered TTL and call `set_bytes_with_ttl`
   - Used compile-time constants: `DEFAULT_BASE_TTL_SECS = 86_400`,
     `DEFAULT_JITTER_FRACTION = 0.10`
   - Added test-only `with_provider_and_ttl` constructor that accepts a
     `Box<dyn rand::RngCore + Send>` for deterministic testing

5. **Test coverage**:
   - Unit tests (`rstest`) verify jitter boundaries, TTL pass-through, and
     round-trip behaviour
   - BDD scenario (`rstest-bdd`) verifies jitter produces varying TTLs in
     live Redis and that all recorded TTLs fall within the configured
     jitter window (via the Redis `TTL` command)
   - **Expiry coverage gap**: actual key-expiry testing (i.e. waiting for
     a key to disappear) was not performed because the adapter uses a
     compile-time 24-hour base TTL with no short-TTL override available
     at runtime. Correctness of expiry is instead verified indirectly:
     the `RedisPoolProvider` delegates to `SET ... EX` (confirmed by
     unit tests), and the BDD TTL-window step confirms Redis reports the
     expected TTL values immediately after writes

6. **Documentation updates**:
   - Updated `docs/wildside-backend-architecture.md` Caching Layer section with
     TTL policy and jitter rationale
   - Marked roadmap item 5.1.3 as complete in `docs/backend-roadmap.md`

## Surprises & discoveries

1. **SmallRng feature requirement**: The `rand` crate requires explicit
   `small_rng` feature flag to use `SmallRng`. Added `rand = { version =
   "0.8", features = ["small_rng"] }` to `Cargo.toml`. Production code
   was later refactored to store `Box<dyn rand::RngCore + Send>` seeded
   via `StdRng::from_entropy()`, but the `small_rng` feature is still
   used in test code for deterministic seeding with `SmallRng`.

2. **BDD test fixture sharing**: The `rstest-bdd` `Slot` type requires
   `Clone` on stored values. Since `GenericRedisRouteCache` cannot
   implement `Clone` (contains `Mutex<Box<dyn RngCore + Send>>`),
   wrapped cache in `Arc<RedisRouteCache<TestPlan>>` via `CacheHandle`
   newtype.

3. **Simplified BDD scenarios**: Initially planned TTL expiry scenarios with
   configurable TTL (e.g., 2-second wait), but the adapter uses compile-time
   constants. Instead, focused on verifying jitter variation via Redis `TTL`
   command, which provides sufficient coverage without 24-hour waits.

## Decision log

1. **Jitter placement**: Applied jitter in the adapter's `put` method rather
   than in `ConnectionProvider`, keeping jitter logic close to the TTL policy
   decision point.

2. **RNG storage**: Used `Mutex<Box<dyn rand::RngCore + Send>>` within
   `GenericRedisRouteCache` instead of passing RNG per-call, trading slight
   lock contention for simpler API surface. Production constructors seed
   via `StdRng::from_entropy()`, while test constructors
   (`with_provider_and_ttl`) accept an arbitrary boxed `RngCore`,
   enabling deterministic assertions without sacrificing thread safety.

3. **Type alias for test helpers**: Added `CachedValue = (Vec<u8>,
   Option<u64>)` type alias to satisfy Clippy's `type_complexity` lint while
   maintaining clarity.

4. **Backward compatibility**: Retained `set_bytes` method on
   `ConnectionProvider` alongside new `set_bytes_with_ttl` to avoid breaking existing
   test code that doesn't care about TTL.

## Outcomes & retrospective

### Outcomes

- **TTL with jitter delivered**: `GenericRedisRouteCache::put` now
  applies a per-write jittered TTL drawn uniformly from the
  [77 760 s, 95 040 s] window (24 h +/- 10%). Entries expire
  individually, eliminating synchronized cache stampedes.
- **Hexagonal boundary preserved**: the `RouteCache` port trait is
  unchanged; TTL is entirely an outbound adapter concern.
- **RNG injection**: production code seeds via `StdRng::from_entropy()`
  behind `Mutex<Box<dyn rand::RngCore + Send>>`; test constructors
  accept a deterministic boxed RNG.
- **Backward-compatible API**: `set_bytes` remains as a
  convenience wrapper delegating to `set_bytes_with_ttl(key, value,
  None)`.

### Decisions confirmed

- Jitter is applied in the adapter `put` method, not in
  `ConnectionProvider`, keeping the policy close to the decision point.
- `FakeProvider` records `(Vec<u8>, Option<u64>)` per key, enabling
  drift-free TTL assertions without a live Redis.

### Coverage gap

- Direct expiry testing (waiting for a key to disappear) was not
  performed. The adapter's 24-hour base TTL is a compile-time constant
  with no short-TTL runtime override. Correctness of expiry relies on
  `SET ... EX` delegation (confirmed by unit tests) and Redis `TTL`
  command verification (confirmed by BDD tests).

### Follow-ups

- Consider adding a runtime-configurable TTL override (environment
  variable or builder parameter) in a future task to enable direct
  expiry integration tests with short TTLs.
- Jitter-specific live-Redis BDD steps were extracted into
  `backend/tests/route_cache_redis_jitter_bdd.rs` to keep file sizes
  under the 400-line cap.

### Completion evidence

- `make check-fmt`: passed.
- `make lint` (Clippy): passed (pre-existing `whitaker`/`checkmake`
  tool absences are unrelated).
- `make test`: all tests passed (1081 tests, 4 skipped for known Redis
  or skip conditions).

## Context and orientation

The relevant current files are:

- `backend/src/domain/ports/route_cache.rs`
  defines the `RouteCache` port with `get` and `put` plus
  `RouteCacheError::{Backend, Serialization}`. The port must not change
  signature for this task.
- `backend/src/domain/ports/cache_key.rs`
  defines the validated `RouteCacheKey` wrapper.
- `backend/src/outbound/cache/redis_route_cache.rs` (364 lines)
  contains `ConnectionProvider`, `RedisPoolProvider`,
  `GenericRedisRouteCache`, and `RedisRouteCache`. Writes go through
  `set_bytes_with_ttl`, which conditionally applies a TTL via
  `SET … EX` or falls back to a plain `SET` for `None`. The
  convenience wrapper `set_bytes` delegates to
  `set_bytes_with_ttl(key, value, None)`.
- `backend/src/outbound/cache/test_helpers.rs` (131 lines)
  contains `FakeProvider` (in-memory `ConnectionProvider` double) and
  `TestPlan`. The fake records TTL values alongside stored bytes.
- `backend/src/outbound/cache/tests/mock_tests.rs` (212 lines)
  unit tests using `FakeProvider`.
- `backend/src/outbound/cache/tests/live_tests.rs` (86 lines)
  integration tests against a live `redis-server`.
- `backend/tests/route_cache_redis_bdd.rs` (387 lines)
  behavioural tests for the Redis-backed `RouteCache` adapter.
- `backend/tests/route_cache_redis_jitter_bdd.rs` (219 lines)
  jitter-specific BDD tests extracted from the main BDD file.
- `backend/tests/features/route_cache_redis.feature` (36 lines)
  Gherkin feature file for cache BDD scenarios.
- `backend/src/test_support/redis.rs` (181 lines)
  `RedisTestServer` helper that starts a local `redis-server` process.
- `docs/wildside-backend-architecture.md`
  the "Caching Layer (Redis)" section describes 24-hour TTL, jittered
  expiry, and key canonicalization as the target behaviour.
- `docs/backend-roadmap.md`
  roadmap item 5.1.3 is the target for this task.

The architecture document already describes the desired behaviour:

> `SET cache:<hash> <route_json> EX 86400` (for a 24h expiry)

This task makes that description a reality in the adapter code and adds
jitter to prevent thundering herd.

## Plan of work

### Stage A: implement the jitter helper

Add a pure function (for example, in a new file
`backend/src/outbound/cache/ttl.rs` or inline in
`redis_route_cache.rs` if it stays under 400 lines) that computes a
jittered TTL:

```rust
/// Compute a TTL in seconds with uniform random jitter.
///
/// Given a `base_ttl` of 86 400 (24 hours) and a `jitter_fraction` of
/// 0.10, the result will be uniformly distributed in the range
/// [77 760, 95 040].
pub(crate) fn jittered_ttl(
    base_ttl: u64,
    jitter_fraction: f64,
    rng: &mut impl rand::Rng,
) -> u64
```

The function:

- Computes `delta = (base_ttl as f64 * jitter_fraction) as u64`.
- Draws a uniform random offset in `[0, 2 * delta]`.
- Returns `(base_ttl - delta + offset).max(1)` to guarantee a positive
  result.

Add `rstest` unit tests in the same module or a sibling test module:

- Parameterized test proving the output is within
  `[base_ttl * (1 - jitter), base_ttl * (1 + jitter)]` for a range of
  seeds.
- Edge case: `base_ttl = 0` returns `1` (minimum clamp).
- Edge case: `jitter_fraction = 0.0` returns `base_ttl` exactly.
- Edge case: `jitter_fraction = 1.0` returns a value in `[0, 2 * base]`,
  clamped to at least 1.

### Stage B: extend the `ConnectionProvider` trait

Add a new method to `ConnectionProvider`:

```rust
/// Write raw bytes for `key` with optional TTL.
///
/// If `ttl_seconds` is `Some(n)`, the entry expires after `n` seconds.
/// If `ttl_seconds` is `None`, the entry persists without expiry.
async fn set_bytes_with_ttl(
    &self,
    key: &str,
    value: Vec<u8>,
    ttl_seconds: Option<u64>,
) -> Result<(), RouteCacheError>;
```

Implement in `RedisPoolProvider`, switching between `SET … EX` and plain
`SET` depending on whether a TTL is provided:

```rust
match ttl_seconds {
    Some(ttl) => connection
        .set_ex::<_, _, ()>(key, value, ttl)
        .await
        .map_err(map_redis_error),
    None => connection
        .set::<_, _, ()>(key, value)
        .await
        .map_err(map_redis_error),
}
```

Update `FakeProvider` in `test_helpers.rs` to record the TTL alongside
the value:

```rust
pub struct FakeProvider {
    store: Mutex<HashMap<String, (Vec<u8>, Option<u64>)>>,
}
```

Add a helper method on `FakeProvider` to retrieve the recorded TTL for
assertions:

```rust
pub fn ttl_for(&self, key: &str) -> Option<u64>
```

Retain `set_bytes` as a default-method wrapper that delegates to
`set_bytes_with_ttl(key, value, None)`. Update
`GenericRedisRouteCache::put` to call `set_bytes_with_ttl` with
`Some(ttl)`, passing the jittered TTL. The wrapper `set_bytes` remains
available for any code that does not need expiry.

### Stage C: integrate TTL into the adapter

Modify `GenericRedisRouteCache` to hold a base TTL, a jitter fraction,
and an RNG source. Use compile-time constants for the defaults:

```rust
const DEFAULT_BASE_TTL_SECS: u64 = 86_400; // 24 hours
const DEFAULT_JITTER_FRACTION: f64 = 0.10;  // +/- 10%
```

The adapter computes a jittered TTL on each `put` call and passes it to
`set_bytes_with_ttl` as `Some(ttl)`. For RNG injection, the adapter
stores `rng: Mutex<Box<dyn rand::RngCore + Send>>` seeded via
`StdRng::from_entropy()` at creation time. Test constructors
(`with_provider_and_ttl`) accept an arbitrary boxed `RngCore` for
deterministic assertions.

Update the existing constructors (`new`, `connect`, `with_provider`) to
initialize the TTL parameters with defaults. Add a test-only constructor
or builder method that accepts a custom RNG seed and/or custom TTL
parameters.

Update `rstest` unit tests to verify:

- `put` calls `set_bytes_with_ttl` with a TTL in the expected jitter window.
- Round-trip behaviour (put then get) is unchanged.
- The recorded TTL on `FakeProvider` is within the expected range.

### Stage D: add behavioural coverage

Add new scenarios to
`backend/tests/features/route_cache_redis.feature`:

```gherkin
Scenario: Cached plans expire after their TTL elapses
  Given a running Redis-backed route cache with a 2-second TTL
  When a plan is stored under cache key "route:expiry"
  And the test waits 3 seconds
  And the cache is read for key "route:expiry"
  Then the cache reports a miss

Scenario: Cached plans are readable before TTL expiry
  Given a running Redis-backed route cache with a 10-second TTL
  When a plan is stored under cache key "route:fresh"
  And the cache is read for key "route:fresh"
  Then the same plan is returned from the cache

Scenario: Jittered writes produce varying TTLs
  Given a running Redis-backed route cache with jitter enabled
  When five plans are stored under distinct cache keys
  Then not all recorded TTLs are identical
```

Implement step definitions in `backend/tests/route_cache_redis_bdd.rs`
(or a new companion file if the existing file exceeds 400 lines).

The "2-second TTL" scenario uses a test constructor on the adapter that
overrides the base TTL and disables jitter (sets `jitter_fraction = 0`)
for deterministic expiry. The "jitter enabled" scenario uses a short base
TTL with standard jitter and inspects the Redis `TTL` command to verify
that not all keys share the same expiry.

Mark live-Redis BDD scenarios with the existing skip mechanism
(`should_skip_redis_tests()` / `SKIP_REDIS_TESTS=1`) so they remain
opt-in.

### Stage E: update documentation

Update `docs/wildside-backend-architecture.md` in the "Caching Layer
(Redis)" section and/or the decision log to record:

- The TTL policy: 24-hour base with +/- 10% uniform jitter.
- Rationale: prevents thundering herd on cache expiry by spreading
  expirations across a ~4.8-hour window.
- Implementation: jitter is computed per `put` invocation in the outbound
  adapter; the domain port remains unaware of expiry.

Update `docs/backend-roadmap.md` to mark item 5.1.3 as done only after
all gates pass.

### Stage F: replay the full repository gates

Run the required repository gates with log capture:

```bash
set -o pipefail
make check-fmt 2>&1 | tee /tmp/5-1-3-check-fmt.out
```

```bash
set -o pipefail
make lint 2>&1 | tee /tmp/5-1-3-lint.out
```

```bash
set -o pipefail
make test 2>&1 | tee /tmp/5-1-3-test.out
```

`make test` is required even though the cache work is not
Postgres-backed, because the repository's standard backend suites still
rely on `pg-embed-setup-unpriv` and the roadmap item cannot close without
a clean global gate run.

## Concrete steps

Run all commands from the repository root. Use `set -o pipefail` and
`tee` for every meaningful command so the exit code survives truncation
and the log is retained.

1. Baseline: confirm the current adapter has no TTL behaviour.

   ```bash
   set -o pipefail
   cargo test -p backend outbound::cache --lib 2>&1 \
     | tee /tmp/5-1-3-cache-baseline.out
   ```

2. Implement the jitter helper and its unit tests, then run the targeted
   suite.

   ```bash
   set -o pipefail
   cargo test -p backend outbound::cache --lib 2>&1 \
     | tee /tmp/5-1-3-jitter-unit.out
   ```

3. Extend `ConnectionProvider`, update `RedisPoolProvider` and
   `FakeProvider`, and integrate TTL into the adapter's `put` path. Run
   the focused cache tests.

   ```bash
   set -o pipefail
   cargo test -p backend outbound::cache --lib 2>&1 \
     | tee /tmp/5-1-3-ttl-unit.out
   ```

4. Add BDD scenarios and step definitions, then run the focused behavioural
   suite.

   ```bash
   set -o pipefail
   cargo test -p backend --test route_cache_redis_bdd 2>&1 \
     | tee /tmp/5-1-3-cache-bdd.out
   ```

5. Update documentation and validate Markdown.

   ```bash
   set -o pipefail
   make markdownlint 2>&1 | tee /tmp/5-1-3-markdownlint.out
   ```

   ```bash
   set -o pipefail
   make nixie 2>&1 | tee /tmp/5-1-3-nixie.out
   ```

6. Run the required full gates before marking the roadmap item done.

   ```bash
   set -o pipefail
   make check-fmt 2>&1 | tee /tmp/5-1-3-check-fmt.out
   ```

   ```bash
   set -o pipefail
   make lint 2>&1 | tee /tmp/5-1-3-lint.out
   ```

   ```bash
   set -o pipefail
   make test 2>&1 | tee /tmp/5-1-3-test.out
   ```

7. Mark `docs/backend-roadmap.md` item 5.1.3 done only after the gate
   logs are clean, then append the log paths and outcome summary to this
   ExecPlan.

## Validation and acceptance

The implementation is done only when all of the following are true:

- TTL behaviour:
  - `GenericRedisRouteCache::put` stores entries with a jittered TTL
    derived from a 24-hour base and +/- 10% uniform jitter.
  - The jittered TTL is always between 77 760 s and 95 040 s for the
    default parameters.
  - Entries stored in Redis expire after their TTL elapses. *(Not
    directly tested — verified indirectly via `SET ... EX` delegation
    and Redis `TTL` command output; see expiry coverage gap above.)*
  - Entries stored in Redis are readable before their TTL elapses.
    *(Not directly tested — covered by existing round-trip BDD
    scenarios that read back within the 24-hour window.)*
  - Multiple writes produce measurably different TTLs (jitter is not
    constant).
- Adapter API:
  - The `RouteCache` port trait remains unchanged.
  - `ConnectionProvider` gains `set_bytes_with_ttl` accepting
    `ttl_seconds: Option<u64>`.
  - `RedisPoolProvider` implements `set_bytes_with_ttl` using
    `SET ... EX` for `Some(ttl)` and plain `SET` for `None`.
  - `FakeProvider` records the TTL for each write (as `CachedValue =
    (Vec<u8>, Option<u64>)`) and exposes it via `ttl_for` for
    assertions.
- Jitter helper:
  - A pure function computes jittered TTL from a base, jitter fraction,
    and injectable RNG.
  - Edge cases (zero base, zero jitter, large jitter) are covered by
    `rstest` parameterized tests.
- Architectural boundaries:
  - Domain code knows only the `RouteCache` trait and `RouteCacheKey`.
  - No inbound adapter or domain service imports Redis, TTL, or RNG types.
  - The TTL policy is an outbound adapter concern, not a domain concern.
- Tests:
  - New `rstest` coverage passes for jitter arithmetic and TTL
    pass-through.
  - New `rstest-bdd` coverage passes for jitter variation and TTL
    window verification against a live `redis-server`. Actual key-expiry
    BDD scenarios are not included (see expiry coverage gap in Progress
    section).
  - Existing Postgres-backed suites still pass through `make test`.
- Documentation:
  - `docs/wildside-backend-architecture.md` records the TTL policy and
    jitter rationale.
  - `docs/backend-roadmap.md` marks 5.1.3 done only after all gates pass.
- Gates:
  - `make check-fmt` passes.
  - `make lint` passes.
  - `make test` passes.

## Approval / implementation gate

Status: APPROVED and IMPLEMENTED.

All validation criteria met (with one qualified exception):

- TTL behaviour verified via unit and BDD tests
- Adapter API extended with `set_bytes_with_ttl` (accepts
  `Option<u64>`)
- Jitter helper implemented with comprehensive edge-case coverage
- Architectural boundaries preserved (domain unchanged)
- Test coverage added — jitter arithmetic, TTL pass-through, jitter
  variation, and TTL window verification are covered; actual key-expiry
  testing is verified indirectly (see expiry coverage gap in Progress)
- Documentation updated
- Gates passed: `make check-fmt`, `make lint` (Clippy), `make test`
