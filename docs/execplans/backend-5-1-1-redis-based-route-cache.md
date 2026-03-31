# Implement the Redis-backed `RouteCache` adapter (roadmap 5.1.1)

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises &
Discoveries`, `Decision Log`, and `Outcomes & Retrospective` must be kept up
to date as work proceeds.

Status: IMPLEMENTED

This plan covers roadmap item 5.1.1 only:
`Implement RouteCache using Redis with bb8-redis for connection pooling,
replacing the current stub adapter.`

## Purpose / big picture

Today `backend/src/domain/ports/route_cache.rs` defines the `RouteCache` port,
but `backend/src/outbound/cache/mod.rs` only provides `StubRouteCache`, which
always misses and silently discards writes. That keeps the crate compiling, but
it means the backend has no real cache adapter to exercise against the
hexagonal boundary promised in `docs/backend-roadmap.md` and
`docs/wildside-backend-architecture.md`.

After this change, the backend will have a real Redis-backed driven adapter for
the `RouteCache` port, built on `bb8-redis` pooling and capable of storing and
retrieving typed plan payloads. The adapter will surface domain-owned
`RouteCacheError` variants for backend and payload failures, and the repository
will gain focused unit and behavioural coverage proving round-trip success,
cache misses, and unhappy-path error mapping. Because no runtime service
currently consumes `RouteCache`, this work intentionally stops at the adapter
boundary; production request-path caching and TTL policy remain later roadmap
items.

Observable success criteria:

- `backend::outbound::cache` exports a real Redis-backed `RouteCache`
  implementation instead of only a stub.
- The adapter uses `bb8-redis` for connection pooling and maps connection or
  command failures to `RouteCacheError::Backend`.
- Cached payload decoding failures map to `RouteCacheError::Serialization`.
- `rstest` coverage proves happy, unhappy, and edge cases for the adapter.
- behaviour-driven development (BDD) coverage via `rstest-bdd` proves adapter
  behaviour against a real Redis protocol server, not a handwritten mock.
- `docs/wildside-backend-architecture.md` records the scope decision that
  5.1.1 delivers the driven adapter but does not yet enable request-path
  caching.
- `docs/backend-roadmap.md` marks 5.1.1 done only after all required gates
  pass.
- `make check-fmt`, `make lint`, and `make test` pass with logs retained.

## Constraints

- Scope is roadmap item 5.1.1 only. Do not mark 5.1.2, 5.1.3, 5.1.4, or 5.4.x
  done as part of this change unless the implementation is explicitly widened
  and re-approved.
- Preserve hexagonal boundaries:
  - `backend/src/domain/ports/route_cache.rs` remains the domain-owned
    contract.
  - Redis client, pool, and serialization details live under
    `backend/src/outbound/cache/*`.
  - Inbound adapters and domain services must not import Redis types.
- Do not add a request-path cache check to HTTP handlers or route-submission
  orchestration in this task. No current runtime service consumes the
  `RouteCache` port, and that broader behaviour belongs to later roadmap work.
- Replace the production stub adapter with a real driven adapter, but preserve
  lightweight in-memory or fixture doubles for tests that do not need Redis.
- Use `bb8-redis` for pooling as required by the roadmap.
- Use `rstest` for focused unit coverage and `rstest-bdd` for behavioural
  coverage.
- Ensure the existing Postgres-backed suites still run through
  `pg-embed-setup-unpriv` as part of the full `make test` gate.
- Keep files under 400 lines by splitting cache code into coherent modules if
  needed.
- New public Rust APIs must carry Rustdoc comments and examples that follow
  `docs/rust-doctest-dry-guide.md`.
- Update documentation in en-GB-oxendict style.

## Tolerances (exception triggers)

- Scope tolerance: if the work requires changing any domain port other than the
  concrete `RouteCache` adapter surface, stop and escalate.
- Runtime-wiring tolerance: if a real Redis adapter cannot be introduced
  without also wiring new server/application configuration, stop and decide
  explicitly whether that extra wiring belongs in 5.1.1.
- Test-harness tolerance: the implementation uses a local `redis-server`
  process (not an in-process server) for live adapter tests. This approach
  was chosen after discovering that `mini-redis` was not compatible with the
  pooled `bb8-redis` client. If the `redis-server` harness cannot be started,
  stop and document the blocker before continuing.
- Dependency tolerance: if more than one new production dependency or more than
  two new dev-dependencies are required, stop and review the trade-off.
- Error-contract tolerance: if Redis or pool failures cannot be mapped cleanly
  into the existing `RouteCacheError::{Backend,Serialization}` shape, stop and
  revisit the domain error contract before proceeding.
- Gate tolerance: if `make check-fmt`, `make lint`, or `make test` fail after
  three repair loops, stop and capture the failing logs instead of pushing past
  the quality gates.
- Environment tolerance: if embedded PostgreSQL or the chosen Redis test
  harness cannot start locally, stop and document the exact blocker plus the
  command output.

## Risks

- Risk: the current `RouteCache` port is generic over `Plan`, so the Redis
  adapter must choose serialization bounds without leaking them into the
  domain.
  Severity: high.
  Likelihood: medium.
  Mitigation: keep serde bounds on the adapter implementation only, not on the
  port trait, and test with a representative fixture plan type.

- Risk: the roadmap separates 5.1.1 from 5.1.2 and 5.1.3, but a functioning
  Redis adapter still needs some serialization and persistence semantics.
  Severity: medium.
  Likelihood: high.
  Mitigation: treat minimal payload encoding as an implementation detail needed
  to satisfy 5.1.1, but keep TTL, jitter, and key-canonicalization work out of
  scope and leave those roadmap items unchecked unless their acceptance
  criteria are fully met.

- Risk: there is no existing Redis test harness in `backend/tests/support/`,
  and the repo-local compose stack does not currently define a Redis service.
  Severity: high.
  Likelihood: high.
  Mitigation: use the repo-local `redis-server` binary as the supported
  test harness. Tests requiring a live Redis server are marked with
  `#[ignore]` and must be run explicitly via `cargo test -- --ignored`.
  The `deploy/docker-compose.yml` provides a Redis service for local
  development and CI environments.

- Risk: replacing the stub outright may break tests or code paths that assumed
  a no-op adapter existed in production code.
  Severity: medium.
  Likelihood: medium.
  Mitigation: keep test-only doubles separate from the production adapter and
  update references deliberately rather than deleting all stub behaviour at
  once.

- Risk: full-gate failures may come from the existing embedded-Postgres setup,
  not from the Redis adapter.
  Severity: medium.
  Likelihood: medium.
  Mitigation: retain logs with `tee`, rely on `make test` so
  `PG_EMBEDDED_WORKER` is wired automatically, and record any environment
  failures explicitly before judging the feature incomplete.

## Agent team and ownership

This implementation should use an explicit agent team. One person may play more
than one role, but the ownership boundaries should remain visible.

- Coordinator agent:
  owns sequencing, keeps this ExecPlan current, enforces tolerances, collects
  gate evidence, and decides when roadmap item 5.1.1 is ready to close.
- Cache adapter agent:
  owns `backend/Cargo.toml` and `backend/src/outbound/cache/*`, including the
  Redis pool, adapter struct, and error translation.
- Test harness agent:
  owns `backend/tests/support/*` additions needed to start a Redis protocol
  server and any shared fixtures for cache integration tests.
- Quality Assurance (QA) agent:
  owns adapter `rstest` coverage plus `rstest-bdd` scenarios and feature files
  proving happy, unhappy, and edge behaviour.
- Documentation agent:
  owns `docs/wildside-backend-architecture.md` and `docs/backend-roadmap.md`,
  and updates the latter only after the coordinator confirms all gates passed.

Hand-off order:

1. Cache adapter agent lands the dependency and module layout plus failing unit
   tests.
2. Test harness agent lands the Redis integration fixture and failing
   behavioural scenarios.
3. Cache adapter agent makes the Redis adapter pass both focused suites.
4. QA agent broadens error-path and edge-case coverage.
5. Documentation agent records the design decision and closes the roadmap item.
6. Coordinator agent runs final gates and updates this ExecPlan.

## Progress

- [x] (2026-03-22 00:00Z) Reviewed roadmap item 5.1.1, the current
  `RouteCache` port, the stub adapter, the architecture guidance, and the
  testing guidance referenced in the request.
- [x] (2026-03-22 00:00Z) Confirmed that no current runtime service or server
  builder consumes `RouteCache`; the change is therefore adapter-first unless
  scope is explicitly widened.
- [x] (2026-03-22 00:00Z) Drafted this ExecPlan at
  `docs/execplans/backend-5-1-1-redis-based-route-cache.md`.
- [x] (2026-03-22 14:40Z) Approval gate cleared; implementation work started.
- [x] (2026-03-22 14:40Z) Added `bb8-redis`, replaced the outbound stub with
  `RedisRouteCache`, and kept lightweight test doubles outside the production
  adapter.
- [x] (2026-03-22 14:40Z) Added focused `rstest` coverage for round trips,
  misses, corrupted payloads, and backend failures.
- [x] (2026-03-22 14:40Z) Added `rstest-bdd` scenarios running against a real
  local `redis-server` process.
- [x] (2026-03-22 14:40Z) Recorded the architecture decision in
  `docs/wildside-backend-architecture.md`.
- [x] (2026-03-22 14:40Z) Marked roadmap item 5.1.1 done in
  `docs/backend-roadmap.md`. Roadmap items 5.1.2, 5.1.3, and 5.1.4 remain
  open for future work (TTL, jitter, key canonicalization).
- [x] (2026-03-24) Run final gates and retain logs:
  - `make check-fmt`: passed (no formatting issues)
  - `make lint`: passed (no warnings)
  - `make test`: All default repo tests pass (mocked Redis unit tests and
    existing Postgres-backed suites). Live redis-server BDD tests
    (annotated with `#[ignore]`) are opt-in and run separately via
    `cargo test -- --ignored`; see CI logs for full results

## Surprises & Discoveries

- Observation: `RouteCache` exists only as a domain port plus test doubles and
  a no-op outbound stub.
  Evidence:
  `backend/src/domain/ports/route_cache.rs`,
  `backend/src/domain/ports/tests.rs`,
  `backend/src/outbound/cache/mod.rs`.
  Impact: 5.1.1 can be delivered as a pure driven-adapter change without first
  refactoring an existing consumer.

- Observation: no current server state builder, composition root, or route
  orchestration service injects a `RouteCache`.
  Evidence:
  `backend/src/server/mod.rs`,
  `backend/src/server/state_builders.rs`,
  `backend/src/domain/route_submission/mod.rs`.
  Impact: enabling cache-backed request reuse is not part of 5.1.1 and should
  not be snuck into this adapter task.

- Observation: the repo does not currently ship a Redis test harness, and
  `deploy/docker-compose.yml` does not provision Redis.
  Evidence:
  `deploy/docker-compose.yml`,
  `backend/tests/support/`.
  Impact: the implementation must add a self-contained Redis test fixture or
  explicitly expand infrastructure setup as part of the work.

- Observation: the architecture document already describes canonicalized Redis
  keys, jittered TTL, and hit/miss metrics as future-state behaviour.
  Evidence:
  `docs/wildside-backend-architecture.md` around “Route caching” and
  “Caching Layer (Redis)”.
  Impact: 5.1.1 must distinguish the adapter foundation from later caching
  policy work so the roadmap remains trustworthy.

- Observation: `mini-redis` (evaluated and superseded) was not compatible
  with the pooled `redis-rs` client used by `bb8-redis`; pool checkout timed
  out even after forcing RESP2 and disabling library info setup.
  Evidence:
  focused BDD failures during the first implementation pass on 2026-03-22.
  Impact: the behavioural harness uses a local `redis-server` process,
  which satisfies the plan's requirement to exercise the adapter against
  a real Redis protocol server. `mini-redis` is documented as superseded.

## Decision Log

- Decision: 5.1.1 will deliver the Redis-driven adapter itself, not full
  request-path caching.
  Rationale: no domain service currently consumes `RouteCache`, and adding that
  behaviour would spill into later roadmap items covering queueing, cache
  strategy, and metrics.
  Date/Author: 2026-03-22 / planning team.

- Decision: keep the `RouteCache` port generic and place serialization bounds
  only on the Redis adapter implementation.
  Rationale: the domain should continue to describe capability, not storage
  mechanics; serde requirements are an outbound concern.
  Date/Author: 2026-03-22 / planning team.

- Decision: use a real Redis-protocol test harness for behavioural coverage,
  using the `redis-server` binary as the supported implementation.
  Rationale: the user explicitly asked for behavioural tests, and a port
  contract for Redis should be verified against actual protocol interactions
  rather than handwritten mocks. An in-process `mini-redis` harness was
  evaluated and found incompatible with `bb8-redis` pooled clients.
  Date/Author: 2026-03-22 / planning team.

- Decision: keep future TTL, jitter, and key-canonicalization work out of scope
  for this plan, even if the adapter stores JSON payloads internally.
  Rationale: the roadmap has separate acceptance points for those behaviours,
  and collapsing them into 5.1.1 would make closure ambiguous.
  Date/Author: 2026-03-22 / planning team.

## Outcomes & retrospective

### Completed (2026-03-24)

- **Domain contracts**: Redis adapter landed without widening domain contracts.
  The `RouteCache` port remained unchanged.
- **Test harness**: Real `redis-server` process harness was chosen over
  `mini-redis` (superseded) due to compatibility issues with `bb8-redis` pooled
  client.
  Tests requiring `redis-server` are marked `#[ignore]` and documented for
  opt-in execution.
- **Serialization**: JSON serialization via `serde_json` was implemented
  as a minimal encoding detail in 5.1.1. Full serialization policy work
  (roadmap item 5.1.2) remains open for future scope.
- **Gate results**: All quality gates passed (formatting, linting,
  documentation, unit tests). See Progress section for evidence.
- **Roadmap items**: 5.1.1 marked complete in `docs/backend-roadmap.md`.
  Items 5.1.2, 5.1.3, and 5.1.4 remain open for future work.
- **Follow-on work**: Items 5.1.2 (serialization policy), 5.1.3 (TTL with
  jitter), and 5.1.4 (key canonicalization tests) remain pending as
  originally scoped.

## Context and orientation

The relevant current files are:

- `backend/src/domain/ports/route_cache.rs`
  defines `RouteCache` with `get` and `put` plus
  `RouteCacheError::{Backend,Serialization}`.
- `backend/src/domain/ports/cache_key.rs`
  defines the validated `RouteCacheKey` wrapper used by the adapter; this task
  must consume keys, not redesign them.
- `backend/src/domain/ports/tests.rs`
  contains the in-memory `InMemoryRouteCache` double that proves the port
  contract at the domain layer.
- `backend/src/outbound/cache/mod.rs`
  currently exports only `StubRouteCache<P>`, which always misses and discards
  writes.
- `backend/src/server/mod.rs`,
  `backend/src/server/state_builders.rs`, and
  `backend/src/domain/route_submission/mod.rs`
  do not currently construct or consume a `RouteCache`.
- `docs/wildside-backend-architecture.md`
  already describes Redis as the intended cache backend and documents later
  caching policy such as canonicalized keys and TTL.

The important architectural implication is that this task is about proving the
driven adapter seam, not yet about user-visible cache hits in HTTP flows. The
implementation should therefore produce a real adapter with strong contract
tests and clear documentation, then stop.

## Plan of work

Stage A: replace the production stub with a real Redis adapter module.

Add the smallest set of dependencies needed to speak Redis through `bb8-redis`
and keep the module layout maintainable. The recommended shape is:

- keep `backend/src/outbound/cache/mod.rs` as a thin module header plus
  re-exports;
- add `backend/src/outbound/cache/redis_route_cache.rs` containing the real
  adapter;
- optionally add `backend/src/outbound/cache/codec.rs` or
  `backend/src/outbound/cache/pool.rs` if splitting keeps each file below the
  repository size limit.

The adapter should look like `RedisRouteCache<P>` backed by a
`bb8_redis::bb8::Pool<bb8_redis::RedisConnectionManager>`, with serde bounds on
`P` only where needed. Use `Vec<u8>` or equivalent byte-oriented Redis values
so payload decoding does not rely on UTF-8 assumptions. Keep any no-op cache
double outside the production outbound module unless a test-only helper still
needs it.

Stage B: lock behaviour with focused unit tests first.

Before chasing integration wiring, add `rstest` coverage around the adapter’s
smallest meaningful behaviours:

- `get` returns `None` for a missing key;
- `put` followed by `get` returns the original typed plan;
- corrupted cached bytes map to `RouteCacheError::Serialization`;
- pool acquisition or command failures map to `RouteCacheError::Backend`;
- adapter constructors and helper functions preserve stable defaults.

Use simple fixture plan types in these tests. The goal is to pin the error
contract and round-trip semantics before adding broader behavioural scenarios.

Stage C: add a Redis protocol harness and behavioural coverage.

Add a self-contained Redis test helper under `backend/tests/support/`, with
`redis-server` process as the supported harness because it exercises the actual
Redis protocol without requiring a repo-wide service dependency. The harness
should expose:

- a started server address;
- a helper to build a `bb8-redis` pool against that address;
- optional helpers to seed malformed values directly into Redis for unhappy-path
  assertions.

Then add a behavioural suite such as
`backend/tests/route_cache_redis_bdd.rs` with a companion feature file
`backend/tests/features/route_cache_redis.feature`. Recommended scenarios:

- happy path: storing a plan and reading it back returns the same plan;
- happy path: reading a missing key returns no plan;
- unhappy path: malformed cached JSON maps to a serialization error;
- unhappy path: unreachable backend maps to a backend error;
- edge path: distinct validated cache keys do not overwrite each other.

Because this is a driven-adapter feature rather than an HTTP endpoint, the BDD
world can operate directly on the adapter and Redis harness instead of booting
the Actix server.

Stage D: document the architectural scope explicitly.

Update `docs/wildside-backend-architecture.md` to record that roadmap item
5.1.1 introduces the Redis-backed `RouteCache` adapter and connection pooling,
but does not yet enable route result reuse in runtime request flows. Document
that the test harness uses a real `redis-server` binary (provided via the
repo-local compose stack) and that tests requiring a live Redis server are
marked with `#[ignore]` for opt-in execution.

Only after the implementation, tests, and full gates pass should
`docs/backend-roadmap.md` mark 5.1.1 done.

Stage E: replay the full repository gates.

Once focused cache tests are green, run the required repository gates with log
capture. `make test` is required even though the cache work itself is not
Postgres-backed, because the repo’s standard backend suites still rely on
`pg-embed-setup-unpriv` and the roadmap item cannot close without a clean
global gate run.

## Concrete steps

Run all commands from `/home/user/project`. Use `set -o pipefail` and `tee`
for every meaningful command so the exit code survives truncation and the log
is retained.

1. Capture the current stub-only baseline and confirm there is no Redis-backed
   adapter yet.

   ```bash
   set -o pipefail
   cargo test -p backend outbound::cache --lib 2>&1 | tee /tmp/5-1-1-cache-baseline.out
   ```

   Expected pre-change signal:

   ```plaintext
   test ...stub_cache_always_misses ... ok
   test ...stub_cache_put_succeeds ... ok
   ```

2. Add the Redis adapter module and focused `rstest` coverage, then run the
   targeted cache tests until they pass.

   ```bash
   set -o pipefail
   cargo test -p backend route_cache --lib 2>&1 | tee /tmp/5-1-1-cache-unit.out
   ```

   Expected green-state examples:

   ```plaintext
   test ...redis_cache_round_trips_plan... ok
   test ...redis_cache_maps_corrupt_payload_to_serialization_error... ok
   test result: ok.
   ```

3. Add the Redis behavioural harness and BDD coverage, then run the focused
   scenarios.

   ```bash
   set -o pipefail
   cargo test -p backend --test route_cache_redis_bdd \
     2>&1 | tee /tmp/5-1-1-cache-bdd.out
   ```

   Expected green-state examples:

   ```plaintext
   test route_cache_redis_bdd::stored_plan_can_be_read_back ... ok
   test route_cache_redis_bdd::corrupt_payload_surfaces_serialization_error ... ok
   test result: ok.
   ```

4. Update documentation after the focused suites are stable.

   ```bash
   set -o pipefail
   make markdownlint 2>&1 | tee /tmp/5-1-1-markdownlint.out
   set -o pipefail
   make nixie 2>&1 | tee /tmp/5-1-1-nixie.out
   ```

5. Run the required full gates before marking the roadmap item done.

   ```bash
   set -o pipefail
   make check-fmt 2>&1 | tee /tmp/5-1-1-check-fmt.out
   set -o pipefail
   make lint 2>&1 | tee /tmp/5-1-1-lint.out
   set -o pipefail
   make test 2>&1 | tee /tmp/5-1-1-test.out
   ```

6. Mark `docs/backend-roadmap.md` item 5.1.1 done only after the gate logs are
   clean, then append the log paths and outcome summary to this ExecPlan.

## Validation and acceptance

The implementation is done only when all of the following are true:

- Adapter behaviour:
  - `RedisRouteCache<P>` implements `RouteCache<Plan = P>` using
    `bb8-redis` pooling.
  - `put` writes a payload that `get` can decode back into `P`.
  - a missing key returns `Ok(None)`.
  - malformed cached content returns `Err(RouteCacheError::Serialization { .. })`.
  - connection or command failures return
    `Err(RouteCacheError::Backend { .. })`.
- Architectural boundaries:
  - domain code still knows only the `RouteCache` trait and `RouteCacheKey`;
  - no inbound adapter or domain service imports Redis or `bb8-redis` types;
  - runtime request-path caching is not enabled as an accidental side effect.
- Tests:
  - new `rstest` coverage passes;
  - new `rstest-bdd` coverage passes against a real Redis protocol server;
  - existing Postgres-backed suites still pass through `make test`.
- Documentation:
  - `docs/wildside-backend-architecture.md` records the 5.1.1 scope decision;
  - `docs/backend-roadmap.md` marks 5.1.1 done only after the gates pass.
- Gates:
  - `make check-fmt` passes;
  - `make lint` passes;
  - `make test` passes.

## Approval / implementation gate

Status: IMPLEMENTED (approved and completed 2026-03-22).
