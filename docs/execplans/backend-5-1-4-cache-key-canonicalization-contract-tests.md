# Add cache-key canonicalization contract tests (roadmap 5.1.4)

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises &
Discoveries`, `Decision Log`, and `Outcomes & Retrospective` must be kept up
to date as work proceeds.

Status: DRAFT

Approval gate: do not begin implementation, do not update
`docs/backend-roadmap.md`, and do not mark roadmap item 5.1.4 done until the
user explicitly approves this ExecPlan.

This plan covers roadmap item 5.1.4 only:
`Add contract tests for cache key canonicalization (sorted themes, rounded
coordinates, Secure Hash Algorithm 256-bit (SHA-256) key format).`

## Purpose / big picture

The Redis-backed `RouteCache` adapter already exists and is covered for
round-trip behaviour, cache misses, malformed payloads, backend failures, and
time-to-live (TTL) jitter. What is still missing is the cache-key contract
promised by `docs/wildside-backend-architecture.md`: semantically equivalent
route requests must collapse onto the same cache key, and materially different
requests must not.

Today the codebase has no production cache-key derivation seam. The domain
type `backend/src/domain/ports/cache_key.rs` only validates raw strings, while
the Redis adapter in `backend/src/outbound/cache/redis_route_cache.rs` simply
passes those strings through to Redis. That means roadmap item 5.1.4 cannot be
completed by tests alone. The work needs a small, domain-owned key builder so
the tests have real behaviour to lock down.

After this change:

- the backend has a domain-owned, deterministic cache-key derivation seam for
  route-cache inputs;
- `rstest` contract tests prove the canonicalization rules for sorted themes,
  rounded coordinates, and versioned SHA-256 key format;
- `rstest-bdd` scenarios prove the observable adapter behaviour that equivalent
  requests share cache entries while materially different requests do not;
- `docs/wildside-backend-architecture.md` records any design clarifications
  required to make the implemented contract unambiguous;
- `docs/backend-roadmap.md` marks only 5.1.4 done, and only after the full
  repository gates pass.

Observable success criteria:

- there is a production API that derives `RouteCacheKey` values from typed
  cache-key input instead of relying on ad hoc string literals;
- equivalent inputs that differ only by theme ordering or coordinate precision
  beyond the fifth decimal place yield identical keys;
- materially different rounded coordinates yield different keys;
- derived keys match the documented `route:v1:<sha256>` format, where
  `<sha256>` is a 64-character lowercase hexadecimal digest;
- live Redis BDD scenarios prove that storing under one canonicalized request
  can be read back through an equivalent reordered request;
- `make check-fmt`, `make lint`, and `make test` pass with logs captured via
  `tee`.

## Reference map and skills

The implementing agent must keep the following references open while working.
These are part of the contract for this plan, not optional background reading.

- Skill: `execplans`
  This plan follows its draft-first workflow, approval gate, living sections,
  and exception-based tolerances.
- Skill: `hexagonal-architecture`
  Cache-key semantics belong in the domain-owned seam, while Redis remains an
  outbound implementation detail. Inbound adapters must not grow Redis-aware
  logic.
- `docs/backend-roadmap.md`
  Source of truth for scope. Only roadmap item 5.1.4 may be closed by this
  work.
- `docs/wildside-backend-architecture.md`
  Source of truth for the cache-key contract: sorted themes, rounded
  coordinates, stable serialization, SHA-256, and `route:v1:<sha256>`.
- `docs/rust-testing-with-rstest-fixtures.md`
  Use reusable fixtures and parameterised tests to keep canonicalization cases
  compact and readable.
- `docs/rstest-bdd-users-guide.md`
  Feature files live under `backend/tests/features/`; assertions belong in
  `Then` steps; fixtures are the preferred state-sharing mechanism.
- `docs/pg-embed-setup-unpriv-users-guide.md`
  The full repository gate still runs PostgreSQL-backed suites. Use the
  Makefile-driven flow rather than bypassing those tests because this feature
  is Redis-focused.
- `docs/rust-doctest-dry-guide.md`
  If public Rust APIs change, add concise Rustdoc examples without duplicating
  setup boilerplate.
- `docs/complexity-antipatterns-and-refactoring-strategies.md`
  Keep canonicalization logic small and explicit. Avoid deeply nested
  branching; extract helpers for sorting, rounding, and hash rendering.
- `docs/documentation-style-guide.md`
  Use sentence-case headings, en-GB-oxendict spelling, 80-column prose
  wrapping, and language-tagged fenced blocks.

## Constraints

- Scope is roadmap item 5.1.4 only. Do not widen into roadmap items 5.1.3,
  5.2.x, 5.3.x, or 5.4.x.
- Preserve the hexagonal boundary:
  - the domain owns route-cache key semantics;
  - outbound cache code owns Redis transport and value serialization;
  - inbound HTTP code must not import or construct Redis-specific cache keys.
- Do not hide the missing production seam behind test-only helpers. The
  canonicalization contract must be expressed by production code.
- Keep the Redis adapter generic over plan payloads. Do not couple
  `RedisRouteCache` to route-request DTOs or inbound JSON payload types.
- Keep existing cache adapter behaviour stable:
  - `RouteCacheError` remains the adapter error surface;
  - JSON payload round-trip behaviour remains unchanged;
  - TTL jitter behaviour remains unchanged.
- Reuse the existing live Redis harness based on `redis-server`. Do not switch
  to a different Redis test backend unless the existing harness is proven
  unusable and the user re-approves the change.
- Use `rstest` for focused contract coverage and `rstest-bdd` for observable
  behavioural coverage.
- Keep files under the repository's 400-line limit by splitting helpers or
  feature support modules when necessary.
- If public APIs are added or changed, document them with Rustdoc comments and
  examples.
- Update documentation in en-GB-oxendict style only after the code contract is
  settled.

## Tolerances (exception triggers)

- Scope tolerance: if satisfying 5.1.4 requires wiring cache lookups into
  route submission or HTTP handlers, stop and escalate. That belongs to later
  roadmap work.
- Interface tolerance: if the best implementation requires changing the
  `RouteCache` trait signature, stop and escalate before proceeding.
- Design tolerance: if the canonicalization contract cannot be expressed by a
  small domain-owned helper adjacent to `RouteCacheKey`, stop and document the
  alternatives before implementing a broader model change.
- Dependency tolerance: if any new production dependency is required, stop and
  escalate. Reuse existing crates such as `serde_json`, `sha2`, and `hex` if
  possible.
- Documentation tolerance: if the architecture document and the intended code
  contract disagree in a way that affects correctness, stop and resolve the
  wording before finalizing tests.
- Gate tolerance: if `make check-fmt`, `make lint`, or `make test` still fail
  after three repair loops, stop and capture the logs rather than continuing.
- Environment tolerance: if Redis or embedded PostgreSQL cannot start locally,
  stop and record the exact failure, including any `SKIP-TEST-CLUSTER`-style
  diagnostics and `/dev/null` state, before continuing.

## Risks

- Risk: the architecture document is ahead of the code and describes
  canonicalization rules that no production seam currently implements.
  Severity: high.
  Likelihood: high.
  Mitigation: begin with a small domain-owned key builder and let the tests
  lock that contract down before any broader cache wiring is attempted.

- Risk: cache-key derivation might be implemented inside the Redis adapter
  because that is where the existing cache tests live.
  Severity: high.
  Likelihood: medium.
  Mitigation: keep request semantics in the domain-facing seam and let the
  Redis adapter continue to consume already-derived `RouteCacheKey` values.

- Risk: the architecture document contains older wording that mentions
  `cache:<hash>` while newer wording specifies `route:v1:<sha256>`.
  Severity: medium.
  Likelihood: high.
  Mitigation: choose one canonical format during implementation, record the
  decision in `docs/wildside-backend-architecture.md`, and align tests and
  prose to that choice.

- Risk: behavioural tests can drift into implementation-detail assertions
  rather than observable outcomes.
  Severity: medium.
  Likelihood: medium.
  Mitigation: use BDD scenarios that store and read plans through equivalent
  or distinct inputs, asserting cache hit or separation rather than internal
  helper structure.

- Risk: full repository gates may fail for reasons unrelated to the Redis
  cache, especially embedded PostgreSQL environment drift.
  Severity: medium.
  Likelihood: medium.
  Mitigation: use the repository-standard Makefile flow, capture logs through
  `tee`, and treat Postgres failures as blocking evidence rather than silently
  skipping them.

## Agent team and ownership

This work must use an explicit agent team. One person may play more than one
role, but the ownership boundaries should stay visible in the implementation
notes and commit history.

- Coordinator agent:
  owns sequencing, keeps this ExecPlan current, enforces tolerances, collects
  gate evidence, and decides when 5.1.4 is ready to close.
- Domain key-contract agent:
  owns the production cache-key derivation seam in
  `backend/src/domain/ports/cache_key.rs` and any adjacent new module if the
  file would otherwise exceed the line limit.
- Unit-test agent:
  owns focused `rstest` coverage in the domain/cache test area, including
  parameterised canonicalization cases and unhappy-path validation.
- Behavioural-test agent:
  owns `backend/tests/route_cache_redis_bdd.rs`,
  `backend/tests/features/route_cache_redis.feature`, and any small support
  additions needed to exercise canonicalized-key behaviour against live Redis.
- Documentation agent:
  owns `docs/wildside-backend-architecture.md`,
  `docs/backend-roadmap.md`, and the final ExecPlan updates after gates pass.

Hand-off order:

1. Coordinator agent confirms the intended production seam and updates this
   plan if the code context changes during approval review.
2. Domain key-contract agent lands the smallest production API that can derive
   canonical `RouteCacheKey` values.
3. Unit-test agent adds failing `rstest` coverage for sorted themes, rounded
   coordinates, key format, and invalid input handling.
4. Behavioural-test agent extends the Redis BDD suite to prove observable
   canonicalization semantics against live Redis.
5. Documentation agent records the final design clarification and marks the
   roadmap item done after the coordinator confirms the gates passed.
6. Coordinator agent runs final gates, updates this ExecPlan, and closes the
   work.

## Implementation outline

### Stage A: settle the production seam

Start by confirming that `backend/src/domain/ports/cache_key.rs` remains the
right home for the domain-owned cache-key contract. The current `RouteCacheKey`
type only validates raw strings, so Stage A adds the smallest possible
derivation seam for canonicalized route-cache inputs.

The seam must:

- accept typed cache-key material rather than arbitrary Redis strings;
- sort theme identifiers deterministically;
- round coordinates to five decimal places before hashing;
- produce a versioned SHA-256 key in `route:v1:<sha256>` form;
- reject malformed or unsupported inputs with a domain-owned validation error,
  not a Redis-specific failure.

If extending `cache_key.rs` would make the file crowded or unclear, split the
derivation logic into a sibling domain module and keep `RouteCacheKey` as the
validated output type.

### Stage B: add focused contract tests with `rstest`

Add focused `rstest` coverage around the production seam before broadening the
BDD harness. Prefer parameterised tests and fixtures over duplicated setup.

These tests must cover at least:

- same themes in different orders yield the same key;
- coordinates differing only after the fifth decimal place yield the same key;
- materially different rounded coordinates yield different keys;
- output format matches `route:v1:<64 lowercase hex characters>`;
- invalid inputs fail before any Redis interaction occurs.

Where the tests need reusable typed inputs, introduce small fixtures rather
than repeating literal setup in every case.

### Stage C: extend Redis behavioural coverage with `rstest-bdd`

Reuse the existing live Redis harness rather than inventing a second one. The
feature file should continue to describe observable behaviour, not helper
internals.

Add scenarios that prove:

- storing a plan under one canonicalized request can be read back through an
  equivalent request with differently ordered themes;
- storing a plan under one canonicalized request can be read back through an
  equivalent request whose coordinates differ only beyond the fifth decimal
  place;
- materially different rounded coordinates do not collide;
- invalid canonicalization input fails cleanly without pretending to be a
  Redis backend outage.

Keep assertions in `Then` steps. If the existing
`backend/tests/features/route_cache_redis.feature` file becomes too noisy,
split canonicalization scenarios into a dedicated feature file under
`backend/tests/features/`.

### Stage D: document the final contract

Update `docs/wildside-backend-architecture.md` once the implemented contract is
clear. The documentation update must:

- confirm the final key format;
- state the exact coordinate precision;
- state that theme identifiers are sorted before hashing;
- distinguish completed cache-key work from later request-path caching and
  invalidation work.

If the implementation resolves the `cache:<hash>` versus `route:v1:<sha256>`
drift, record that explicitly in the design-decision section.

After the full gates pass, mark `docs/backend-roadmap.md` item 5.1.4 as done.

### Stage E: full validation and evidence capture

Run focused tests first while iterating, then run the mandatory repository
gates with log capture.

Suggested focused commands during implementation:

```bash
set -o pipefail && cargo test -p backend route_cache --lib \
  2>&1 | tee /tmp/backend-5-1-4-unit.log
set -o pipefail && cargo test -p backend --test route_cache_redis_bdd -- --nocapture \
  2>&1 | tee /tmp/backend-5-1-4-bdd.log
```

Mandatory closure commands:

```bash
set -o pipefail && make check-fmt \
  2>&1 | tee /tmp/backend-5-1-4-check-fmt.log
set -o pipefail && make lint \
  2>&1 | tee /tmp/backend-5-1-4-lint.log
set -o pipefail && make test \
  2>&1 | tee /tmp/backend-5-1-4-test.log
```

If `make test` fails because embedded PostgreSQL cannot start, record the exact
error output, inspect `/dev/null`, and keep the failure evidence in the
ExecPlan rather than papering over it.

## Progress

- [x] (2026-04-21 00:00Z) Reviewed roadmap item 5.1.4 and adjacent cache
  roadmap entries.
- [x] (2026-04-21 00:00Z) Reviewed the current Redis cache adapter, cache test
  helpers, live Redis BDD harness, and `RouteCacheKey` domain type.
- [x] (2026-04-21 00:00Z) Reviewed the architecture and testing guidance named
  in the request.
- [x] (2026-04-21 00:00Z) Confirmed that no production cache-key derivation
  seam exists yet, so 5.1.4 requires a small amount of production code to
  support the contract tests.
- [x] (2026-04-21 00:00Z) Drafted this ExecPlan at
  `docs/execplans/backend-5-1-4-cache-key-canonicalization-contract-tests.md`.
- [ ] Approval gate: user approves the ExecPlan.
- [ ] Stage A: settle and implement the domain-owned canonical key seam.
- [ ] Stage B: add focused `rstest` contract coverage.
- [ ] Stage C: extend live Redis `rstest-bdd` scenarios.
- [ ] Stage D: update architecture documentation and mark roadmap item 5.1.4
  done.
- [ ] Stage E: run final gates and capture evidence.

## Surprises & discoveries

2026-04-21: the current `RouteCacheKey` type only validates raw strings. It
does not yet express the canonical route-cache request contract that the
architecture document describes.

2026-04-21: the Redis adapter and its tests are already healthy for payload
round-trip, miss, corruption, backend failure, and TTL jitter. The missing
piece is the production key-derivation seam, not the Redis harness.

2026-04-21: `docs/wildside-backend-architecture.md` describes
`route:v1:<sha256>` as the canonical key shape, but older wording elsewhere in
the same document still refers to `cache:<hash>`. The implementation must
resolve this drift rather than codifying both.

2026-04-21: full repository validation still depends on embedded PostgreSQL
through `make test`, even though 5.1.4 is Redis-focused. The implementation
must therefore treat `pg-embed-setup-unpriv` readiness as part of the closure
criteria.

## Decision log

**Decision A1 (2026-04-21):** 5.1.4 remains adapter-focused, but it is not a
tests-only change.

Rationale: there is no production cache-key derivation seam today. Adding only
tests would lock in test scaffolding rather than real behaviour. The minimum
honest implementation is a small production seam plus contract tests around it.

Trade-off: this slightly broadens the code touched by a roadmap item phrased as
"add contract tests", but it keeps the roadmap truthful and the tests tied to
real behaviour.

**Decision A2 (2026-04-21):** the canonicalization seam should be domain-owned,
not Redis-owned.

Rationale: sorting theme identifiers and rounding coordinates are request
semantics, not Redis transport concerns. Keeping them adjacent to
`RouteCacheKey` preserves the hexagonal boundary and leaves
`RedisRouteCache` generic over plan payloads.

Trade-off: the domain gains a small helper or input type, but the adapter stays
simple and reusable.

**Decision A3 (2026-04-21):** behavioural coverage will reuse the existing live
Redis BDD harness.

Rationale: the codebase already has a working `redis-server`-backed harness,
runtime skip behaviour, and Redis support modules. Reusing that harness keeps
the change focused and lowers test-infrastructure risk.

Trade-off: live Redis scenarios remain dependent on local `redis-server`
availability, but that dependency is already established elsewhere in the
cache suite.

## Outcomes & retrospective

Pending implementation. This section must be updated after approval, code
changes, documentation updates, and final gate runs are complete.
