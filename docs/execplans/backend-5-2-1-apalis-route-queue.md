# Implement the Apalis-backed `RouteQueue` adapter (roadmap 5.2.1)

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: BLOCKED (Dependency Conflict)

This plan covers roadmap item 5.2.1 only:
`Implement RouteQueue using Apalis with PostgreSQL backend, replacing the
current stub adapter.`

## Purpose / big picture

Today `backend/src/domain/ports/route_queue.rs` defines the `RouteQueue` port
with a single async method `enqueue`, but
`backend/src/outbound/queue/mod.rs` only provides `StubRouteQueue`, which
discards every job with a one-time warning. That keeps the crate compiling, but
means no enqueued work is persisted or available for downstream processing.

After this change, the backend will have a real Apalis-backed driven adapter
for the `RouteQueue` port, built on `apalis-postgres` with PostgreSQL storage.
The adapter will accept typed plan payloads, serialize them into the Apalis job
table, and surface domain-owned `JobDispatchError` variants when the queue
infrastructure is unavailable or rejects a job. The repository will gain
focused unit and behavioural test coverage proving enqueue success, queue
unavailability errors, and rejected-job error mapping against a real embedded
PostgreSQL instance via `pg-embedded-setup-unpriv`.

Because no runtime service currently dispatches to `RouteQueue` (the dispatch
points in `backend/src/domain/route_submission/mod.rs` are gated behind
`TODO(#276)`), this work intentionally stops at the adapter boundary.
Production request-path queue dispatch, worker consumption, job struct
definitions (roadmap 5.2.2), retry policies (5.2.3), and trace propagation
(5.2.4) remain later roadmap items.

Observable success criteria:

- `backend::outbound::queue` exports a real Apalis-backed `RouteQueue`
  implementation instead of only a stub.
- The adapter uses `apalis-postgres` with `PostgresStorage` for job
  persistence and maps connection or dispatch failures to
  `JobDispatchError::Unavailable`.
- Job serialization failures (if the plan type cannot be encoded) map to
  `JobDispatchError::Rejected`.
- `rstest` coverage proves happy, unhappy, and edge cases for the adapter.
- Behavioural (BDD) coverage via `rstest-bdd` proves adapter behaviour against
  a real embedded PostgreSQL instance, not a handwritten mock.
- `docs/wildside-backend-architecture.md` records the scope decision that
  5.2.1 delivers the driven adapter but does not yet enable request-path queue
  dispatch or worker consumption.
- `docs/backend-roadmap.md` marks 5.2.1 done only after all required gates
  pass.
- `make check-fmt`, `make lint`, and `make test` pass with logs retained.

## Constraints

- Scope is roadmap item 5.2.1 only. Do not mark 5.2.2, 5.2.3, 5.2.4, or
  5.3.x done as part of this change unless the implementation is explicitly
  widened and re-approved.
- Preserve hexagonal boundaries:
  - `backend/src/domain/ports/route_queue.rs` remains the domain-owned
    contract. The `RouteQueue` trait and `JobDispatchError` enum must not be
    widened to accommodate Apalis-specific concerns.
  - Apalis client, storage, and serialization details live under
    `backend/src/outbound/queue/*`.
  - Inbound adapters and domain services must not import Apalis types.
- Do not add queue dispatch calls to the `RouteSubmissionServiceImpl` or any
  HTTP handler in this task. The `TODO(#276)` markers at
  `backend/src/domain/route_submission/mod.rs` lines 241 and 295 are
  intentionally left for a later integration step.
- Do not implement worker consumption (`Monitor`, `WorkerBuilder`) in this
  task. The adapter covers the "push" side only; the "pull" side belongs to
  roadmap items 5.2.2 through 5.3.1.
- Replace the production stub adapter with a real driven adapter, but preserve
  lightweight in-memory or fixture doubles for tests that do not need
  PostgreSQL.
- Use `apalis-postgres` (from the `apalis` 1.x release candidate family) for
  PostgreSQL-backed job storage as specified by the roadmap.
- Use `rstest` for focused unit coverage and `rstest-bdd` for behavioural
  coverage.
- Ensure the existing Postgres-backed suites still run through
  `pg-embedded-setup-unpriv` as part of the full `make test` gate.
- Keep files under 400 lines by splitting queue code into coherent modules if
  needed.
- New public Rust APIs must carry Rustdoc comments and examples that follow
  `docs/rust-doctest-dry-guide.md`.
- Update documentation in en-GB-oxendict style.
- Follow the adapter pattern established by `RedisRouteCache` in
  `backend/src/outbound/cache/redis_route_cache.rs`: use a
  `ConnectionProvider`-style trait to abstract the storage backend, keep serde
  bounds on the adapter (not the port), and provide a `FakeProvider` for unit
  tests.

## Tolerances (exception triggers)

- Scope tolerance: if the work requires changing the `RouteQueue` port trait
  signature or `JobDispatchError` enum beyond what is already defined, stop
  and escalate.
- Runtime-wiring tolerance: if a real Apalis adapter cannot be introduced
  without also wiring new server/application configuration or modifying
  `state_builders.rs`, stop and decide explicitly whether that extra wiring
  belongs in 5.2.1.
- Dependency tolerance: adding `apalis`, `apalis-core`, `apalis-sql`, and
  `apalis-postgres` (plus `sqlx` if not already present) is expected. If more
  than two additional production dependencies beyond these are required, stop
  and review the trade-off.
- Error-contract tolerance: if Apalis or SQLx failures cannot be mapped
  cleanly into the existing `JobDispatchError::{Unavailable, Rejected}` shape,
  stop and revisit the domain error contract before proceeding.
- Test-harness tolerance: the implementation uses the existing
  `pg-embedded-setup-unpriv` infrastructure for live adapter tests (the same
  harness used by Diesel repository tests). If the embedded PostgreSQL cluster
  cannot be started, stop and document the blocker before continuing.
- Migration tolerance: if Apalis requires database migrations that conflict
  with or duplicate existing Diesel migrations, stop and escalate.
  `PostgresStorage::setup()` creates its own tables; this must be verified to
  coexist safely with the existing Diesel-managed schema.
- Gate tolerance: if `make check-fmt`, `make lint`, or `make test` fail after
  three repair loops, stop and capture the failing logs instead of pushing past
  the quality gates.
- Environment tolerance: if embedded PostgreSQL cannot start locally, stop and
  document the exact blocker plus the command output.
- File-size tolerance: if any single source file exceeds 400 lines, split it
  into coherent sub-modules before proceeding.

## Risks

- Risk: Apalis 1.x is in release-candidate status, not a stable 1.0 release.
  The API surface may change between release candidates.
  Severity: medium.
  Likelihood: medium.
  Mitigation: pin the `apalis-postgres` version tightly in `Cargo.toml` (for
  example, `"1.0.0-rc.6"`) and document the pinned version in the decision
  log. If a breaking change is encountered, evaluate whether an older RC or
  the 0.7.x stable line is more appropriate.

- Risk: `PostgresStorage::setup()` creates its own tables in the connected
  database. These tables may conflict with existing Diesel-managed migrations
  or with `pg-embedded-setup-unpriv` template databases.
  Severity: high.
  Likelihood: medium.
  Mitigation: call `PostgresStorage::setup()` after Diesel migrations in test
  setup, and verify that the Apalis tables do not collide with existing table
  names. If collisions occur, use a separate PostgreSQL schema or database for
  the queue.

- Risk: the `RouteQueue` port is generic over `Plan` via an associated type,
  so the Apalis adapter must choose serialization bounds without leaking them
  into the domain.
  Severity: high.
  Likelihood: medium.
  Mitigation: keep `Serialize + DeserializeOwned` bounds on the adapter
  implementation only (not on the port trait), following the pattern
  established by `RedisRouteCache`. Test with a representative fixture plan
  type.

- Risk: Apalis uses `sqlx` for PostgreSQL access whereas the existing
  repository layer uses `diesel-async`. Two connection pool implementations
  coexisting may introduce complexity.
  Severity: medium.
  Likelihood: high.
  Mitigation: the Apalis adapter owns its own `sqlx::PgPool`, which is
  separate from the Diesel `bb8` pool. Both connect to the same PostgreSQL
  instance but through independent pool implementations. Document this
  dual-pool arrangement in the architecture doc.

- Risk: the roadmap text says "Apalis with PostgreSQL backend", but the
  architecture document's code samples reference Redis as the job broker. The
  implementation must follow the roadmap, not the older architecture prose.
  Severity: low.
  Likelihood: low.
  Mitigation: use `apalis-postgres` as specified by the roadmap. Record the
  divergence from older architecture prose in the decision log and update the
  architecture document to reflect the chosen backend.

- Risk: full-gate failures may come from the existing embedded-Postgres setup,
  not from the Apalis adapter.
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
  gate evidence, and decides when roadmap item 5.2.1 is ready to close.

- Queue adapter agent:
  owns `backend/Cargo.toml` dependency additions and
  `backend/src/outbound/queue/*`, including the Apalis storage wrapper, adapter
  struct, error translation, and connection provider trait.

- Test harness agent:
  owns `backend/tests/support/*` additions needed to provision an
  Apalis-compatible PostgreSQL database within the existing
  `pg-embedded-setup-unpriv` infrastructure, and any shared fixtures for queue
  integration tests.

- Quality Assurance (QA) agent:
  owns adapter `rstest` coverage plus `rstest-bdd` scenarios and feature files
  proving happy, unhappy, and edge behaviour.

- Documentation agent:
  owns `docs/wildside-backend-architecture.md` and `docs/backend-roadmap.md`,
  and updates the latter only after the coordinator confirms all gates passed.

Hand-off order:

1. Queue adapter agent adds the dependencies and module layout plus failing
   unit tests.
2. Test harness agent lands the Postgres integration fixture additions and
   failing behavioural scenarios.
3. Queue adapter agent makes the Apalis adapter pass both focused suites.
4. QA agent broadens error-path and edge-case coverage.
5. Documentation agent records the design decision and closes the roadmap item.
6. Coordinator agent runs final gates and updates this ExecPlan.

## Progress

- [x] Review roadmap item 5.2.1, the current `RouteQueue` port, the stub
  adapter, the architecture guidance, and the testing guides.
- [x] Confirm that no current runtime service or server builder dispatches to
  `RouteQueue`; verify the change is adapter-first.
- [x] Draft this ExecPlan at
  `docs/execplans/backend-5-2-1-apalis-route-queue.md`.
- [x] Await approval gate.
- [x] Run baseline queue tests (`/tmp/5-2-1-queue-baseline.out` - 1 passing test confirmed)
- [BLOCKED] Add `apalis-postgres` and related dependencies to `backend/Cargo.toml`.
  - Attempted with apalis-postgres 1.0.0-rc.6, apalis-sql 0.7.x, multiple resolution strategies
  - Blocked by libsqlite3-sys native library conflict (see Surprises & discoveries #1)
  - Log: `/tmp/5-2-1-check-deps.out`, `/tmp/5-2-1-check-deps-0.7.out`, `/tmp/5-2-1-check-deps-workspace-patch.out`
- [ ] Create `backend/src/outbound/queue/apalis_route_queue.rs` with the
  adapter struct, connection provider trait, and error mapping.
- [ ] Create `backend/src/outbound/queue/test_helpers.rs` with a fake provider
  for unit tests.
- [ ] Add focused `rstest` unit tests in
  `backend/src/outbound/queue/apalis_route_queue.rs` (or a sibling `tests`
  module) covering enqueue round-trip, unavailable backend, and rejected job
  paths.
- [ ] Extend `backend/tests/support/` with Apalis PostgreSQL setup helpers
  that reuse the `pg-embedded-setup-unpriv` cluster.
- [ ] Create `backend/tests/features/route_queue_apalis.feature` with BDD
  scenarios.
- [ ] Create `backend/tests/route_queue_apalis_bdd.rs` with step definitions
  exercising the adapter against a real PostgreSQL instance.
- [ ] Update `docs/wildside-backend-architecture.md` with the 5.2.1 scope
  decision and dual-pool documentation.
- [ ] Run `make check-fmt` and capture the log.
- [ ] Run `make lint` and capture the log.
- [ ] Run `make test` and capture the log.
- [ ] Mark `docs/backend-roadmap.md` item 5.2.1 done after all gates pass.

## Surprises & discoveries

### Discovery 1: Dependency conflict blocks apalis-postgres integration (2026-04-03)

**Issue**: Cannot add `apalis-postgres` (either 1.0.0-rc.6 or apalis-sql 0.7.x) due to a `libsqlite3-sys` native library version conflict:

- `wildside-data` (git dependency at rev 894aa38) depends on `rusqlite` 0.31 → `libsqlite3-sys` 0.28
- `pg-embed-setup-unpriv` 0.5.0 (used for test infrastructure) depends on `postgresql_embedded` 0.20.1 → `sqlx` 0.8.6 → `libsqlite3-sys` 0.30+
- Cargo enforces that only one version of a native library (`sqlite3`) can be linked per binary

**Attempted resolutions (all failed)**:
- Adjusting version constraints for `url`, `hex`, `postgresql_embedded`
- Disabling sqlx default features
- Explicitly adding `rusqlite` 0.32 or `libsqlite3-sys` 0.30 to backend/Cargo.toml
- Using `[patch.crates-io]` at workspace level (patches require different source, can't patch crates.io with crates.io)
- Using `[workspace.dependencies]` (doesn't override git dependency locks)
- Switching to apalis 0.7.x stable (still blocked by pg-embed-setup-unpriv → sqlx 0.8)

**Root cause**: The git dependency `wildside-data` is locked to a specific revision that uses rusqlite 0.31, and Cargo cannot override transitive dependencies of git sources.

**Options forward**:

1. **Update wildside-engine upstream**: Modify the wildside-engine repository to use rusqlite 0.32+, then update the git revision in this project
   - Pros: Clean resolution, no workarounds
   - Cons: Requires upstream coordination, blocks this task

2. **Use apalis-core directly without apalis-postgres**: Implement a custom PostgreSQL storage adapter using Diesel instead of sqlx
   - Pros: Avoids sqlx entirely, keeps Diesel as single DB access layer
   - Cons: Significantly more implementation work, diverges from roadmap plan

3. **Use an alternate job queue library**: Evaluate alternatives like `fang`, `tokio-cron-scheduler`, or custom implementation
   - Pros: May avoid dependency conflicts
   - Cons: Deviates from approved roadmap, requires re-evaluation of architecture

4. **Defer 5.2.1 until wildside-data conflict is resolved**: Mark this task as blocked and continue with other roadmap items
   - Pros: Clean path forward once unblocked
   - Cons: Delays queue functionality

**Recommendation**: Escalate to project stakeholders for decision. This is a structural dependency conflict that cannot be resolved through standard Cargo mechanisms without either upstream changes or significant architectural divergence from the approved plan.

## Decision log

- Decision: 5.2.1 will deliver the Apalis-driven adapter itself, not
  request-path queue dispatch or worker consumption.
  Rationale: no domain service currently dispatches to `RouteQueue` (gated
  behind `TODO(#276)` in `route_submission`), and adding that behaviour would
  spill into later roadmap items covering job structs, retry policies, and
  worker deployment.
  Date/Author: 2026-04-03 / planning team.

- Decision: use `apalis-postgres` (1.0.0-rc family) rather than
  `apalis-sql` with the `postgres` feature flag.
  Rationale: `apalis-postgres` is the dedicated PostgreSQL crate with
  purpose-built storage types (`PostgresStorage`, `PostgresStorageWithListener`)
  and is the recommended path for PostgreSQL-backed queues in the Apalis
  ecosystem. It provides `NOTIFY`/`SKIP LOCKED` support and heartbeat-based
  orphan recovery.
  Date/Author: 2026-04-03 / planning team.

- Decision: use `sqlx::PgPool` for the Apalis adapter, coexisting with the
  Diesel `bb8` pool used by repository adapters.
  Rationale: Apalis is built on SQLx and expects a `sqlx::PgPool`. Attempting
  to share the Diesel pool would require a brittle adapter layer. Both pools
  connect to the same PostgreSQL instance with independent lifecycle
  management. This dual-pool arrangement mirrors how the Redis cache adapter
  uses its own `bb8-redis` pool independently of Diesel.
  Date/Author: 2026-04-03 / planning team.

- Decision: follow the `RedisRouteCache` adapter pattern — use a
  `QueueProvider` trait to abstract the Apalis storage backend, keeping the
  adapter generic over both the plan type and the provider.
  Rationale: this pattern is proven in the codebase, enables unit testing with
  a `FakeProvider`, and keeps the adapter decoupled from Apalis internals.
  Date/Author: 2026-04-03 / planning team.

- Decision: the Apalis adapter's PostgreSQL tables are created by
  `PostgresStorage::setup()` at connection time, not by Diesel migrations.
  Rationale: Apalis owns its table schema and the `setup()` call is
  idempotent. Mixing Apalis schema into Diesel migrations would create a
  coupling between the two ORMs. The adapter calls `setup()` during
  construction, and tests call it during harness provisioning after Diesel
  migrations complete.
  Date/Author: 2026-04-03 / planning team.

- Decision: use PostgreSQL rather than AMQP (RabbitMQ / LavinMQ) as the
  queue backend for 5.2.1, on the explicit condition that the `QueueProvider`
  abstraction provides sufficient isolation to migrate to an AMQP backend at
  a later date should volume warrant it.
  Rationale: PostgreSQL is already provisioned in production and in the test
  harness (`pg-embedded-setup-unpriv`), so the adapter can be tested and
  deployed with zero new infrastructure. AMQP would require a new broker
  service in production, a new test harness (no embedded RabbitMQ equivalent
  exists for Rust), and depends on the less mature `apalis-amqp` crate. The
  hexagonal boundary — specifically the `QueueProvider` trait and the
  domain-owned `RouteQueue` port — ensures that swapping to an
  `ApalisAmqpProvider` is a contained adapter change: the domain port, all
  `FakeQueueProvider` unit tests, and consuming services remain untouched.
  If queue throughput or routing/fanout requirements outgrow PostgreSQL
  `NOTIFY`/`SKIP LOCKED`, the migration path is to implement a new provider
  behind the same trait and update the composition root.
  Date/Author: 2026-04-03 / planning team.

- Decision: the architecture document will be updated to reflect that the
  queue backend is PostgreSQL (via `apalis-postgres`), not Redis, for job
  storage. The older architecture prose references Redis as the job broker;
  the roadmap explicitly specifies PostgreSQL.
  Rationale: the roadmap is the authoritative source for delivery scope. The
  architecture document must be updated to match.
  Date/Author: 2026-04-03 / planning team.

## Outcomes & retrospective

(To be populated upon completion.)

## Context and orientation

The relevant current files are:

- `backend/src/domain/ports/route_queue.rs`
  defines the `RouteQueue` trait with a single async method `enqueue` and an
  associated type `Plan: Send + Sync`. It also defines `JobDispatchError` with
  two variants: `Unavailable { message: String }` (queue infrastructure is
  down) and `Rejected { message: String }` (the job could not be acknowledged
  or persisted). Both variants have auto-generated constructors via the
  `define_port_error!` macro (for example, `JobDispatchError::unavailable("…")`
  and `JobDispatchError::rejected("…")`).

- `backend/src/outbound/queue/mod.rs`
  currently exports only `StubRouteQueue<P>`, which is generic over any
  `P: Send + Sync`, implements `RouteQueue` with a no-op `enqueue` that logs a
  warning once per process. The stub has a small unit test confirming enqueue
  succeeds.

- `backend/src/domain/route_submission/mod.rs`
  contains two `TODO(#276)` markers (lines 241 and 295) where queue dispatch
  will eventually be wired. This integration is out of scope for 5.2.1.

- `backend/src/server/state_builders.rs`
  does not currently construct or inject a `RouteQueue`. The server composition
  root is unchanged by this task.

- `backend/src/outbound/cache/redis_route_cache.rs`
  is the reference implementation for the adapter pattern: it defines a
  `ConnectionProvider` trait, a `RedisPoolProvider` concrete implementation,
  and a `GenericRedisRouteCache<P, C>` struct parameterised over the plan type
  and provider. Error mapping consolidates all infrastructure failures into
  domain error variants. A `FakeProvider` in
  `backend/src/outbound/cache/test_helpers.rs` enables unit testing without
  Redis.

- `backend/tests/support/embedded_postgres.rs`
  provides `provision_template_database()` for test database creation using
  `pg-embedded-setup-unpriv`. This harness will be reused for the queue
  adapter's integration tests.

- `docs/wildside-backend-architecture.md`
  describes the hexagonal architecture, the `RouteQueue` port, and the
  outbound adapter pattern. It will be updated to document the Apalis adapter
  scope and the dual-pool (SQLx + Diesel) arrangement.

- `docs/backend-roadmap.md`
  lists 5.2.1 as the first queue adapter task. The item will be marked done
  only after all gates pass.

Key terminology:

- **Apalis**: a Rust background job processing library that uses tower
  `Service` semantics. Jobs are defined as serializable structs; workers
  consume them from a storage backend.
- **`apalis-postgres`**: the PostgreSQL storage backend for Apalis, using SQLx
  and `NOTIFY`/`SKIP LOCKED` for reliable job delivery.
- **`PostgresStorage`**: the concrete Apalis type that implements job
  enqueueing and consumption against a PostgreSQL database.
- **`pg-embedded-setup-unpriv`**: a crate used in this repository to provision
  ephemeral PostgreSQL clusters for integration testing without requiring
  system-level PostgreSQL installation.
- **Driven adapter**: an outbound adapter that implements a domain port,
  translating domain calls into infrastructure operations (for example,
  `enqueue` → Apalis `push_request`).
- **Connection provider**: a trait abstracting the storage backend so the
  adapter can be tested with fakes in unit tests and with real PostgreSQL in
  integration tests.

## Plan of work

Stage A: add dependencies and scaffold the adapter module.

Add `apalis-core` and `apalis-postgres` to `backend/Cargo.toml` as production
dependencies, plus `sqlx` with the `postgres` and `runtime-tokio` features (if
not already present). The `apalis-postgres` crate transitively brings in
`apalis-core` and `sqlx`, but explicit entries ensure version pinning. Pin
`apalis-postgres` to a specific 1.0.0 release candidate version.

Restructure `backend/src/outbound/queue/` from a single `mod.rs` into a
multi-file module:

- keep `backend/src/outbound/queue/mod.rs` as a thin module header with
  re-exports;
- add `backend/src/outbound/queue/stub_route_queue.rs` containing the existing
  `StubRouteQueue` (moved from `mod.rs`);
- add `backend/src/outbound/queue/apalis_route_queue.rs` containing the real
  adapter;
- add `backend/src/outbound/queue/test_helpers.rs` (gated behind `#[cfg(test)]`)
  containing a `FakeQueueProvider` for unit tests.

The adapter should follow this shape:

```rust
// backend/src/outbound/queue/apalis_route_queue.rs

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use std::marker::PhantomData;

use crate::domain::ports::{JobDispatchError, RouteQueue};

/// Abstracts the queue storage backend for testability.
#[async_trait]
pub(crate) trait QueueProvider: Send + Sync {
    /// Push a serialized job payload into the queue.
    async fn push_job(&self, payload: Vec<u8>) -> Result<(), JobDispatchError>;
}

/// Apalis-backed `RouteQueue` adapter using PostgreSQL storage.
#[derive(Debug, Clone)]
pub struct GenericApalisRouteQueue<P, Q> {
    provider: Q,
    _plan: PhantomData<fn() -> P>,
}

/// Production type alias with the real Apalis PostgreSQL provider.
pub type ApalisRouteQueue<P> =
    GenericApalisRouteQueue<P, ApalisPostgresProvider>;

#[async_trait]
impl<P, Q> RouteQueue for GenericApalisRouteQueue<P, Q>
where
    P: Serialize + DeserializeOwned + Send + Sync,
    Q: QueueProvider,
{
    type Plan = P;

    async fn enqueue(
        &self,
        plan: &Self::Plan,
    ) -> Result<(), JobDispatchError> {
        let payload = serde_json::to_vec(plan)
            .map_err(|e| JobDispatchError::rejected(e.to_string()))?;
        self.provider.push_job(payload).await
    }
}
```

The `ApalisPostgresProvider` wraps `apalis_postgres::PostgresStorage` and maps
its errors to `JobDispatchError` variants:

```rust
// backend/src/outbound/queue/apalis_route_queue.rs (continued)

use apalis_postgres::PostgresStorage;
use sqlx::PgPool;

/// Real provider backed by Apalis PostgreSQL storage.
#[derive(Debug, Clone)]
pub struct ApalisPostgresProvider {
    storage: PostgresStorage<Vec<u8>>,
}

#[async_trait]
impl QueueProvider for ApalisPostgresProvider {
    async fn push_job(
        &self,
        payload: Vec<u8>,
    ) -> Result<(), JobDispatchError> {
        self.storage
            .push_request(payload)
            .await
            .map_err(|e| JobDispatchError::unavailable(e.to_string()))
    }
}
```

Note: the exact Apalis API may differ from what is shown above. The
implementation agent must consult the `apalis-postgres` 1.0.0-rc documentation
at build time to determine the correct method names (`push_request`,
`push_job`, `enqueue`, etc.) and adjust accordingly. The key invariant is that
the provider maps all Apalis/SQLx errors to `JobDispatchError::Unavailable`
and all serialization errors to `JobDispatchError::Rejected`.

Stage B: lock behaviour with focused unit tests first.

Before chasing integration wiring, add `rstest` coverage around the adapter's
smallest meaningful behaviours, using the `FakeQueueProvider`:

- `enqueue` with a valid plan succeeds and the fake provider receives the
  serialized payload;
- `enqueue` with a plan that fails serialization returns
  `JobDispatchError::Rejected`;
- `enqueue` when the provider returns an error returns
  `JobDispatchError::Unavailable`;
- adapter constructors preserve stable defaults;
- the stub adapter continues to work unchanged (regression).

Use a simple fixture plan type (for example, a `TestPlan` struct deriving
`Debug`, `Clone`, `PartialEq`, `Eq`, `Serialize`, and `Deserialize` with a
single `name: String` field) in these tests. The goal is to pin the error
contract and round-trip semantics
before adding broader behavioural scenarios.

Stage C: add PostgreSQL integration harness and behavioural coverage.

Extend the existing `backend/tests/support/` infrastructure to provision an
Apalis-compatible database. The approach is:

1. Reuse `pg-embedded-setup-unpriv` to create an embedded PostgreSQL cluster
   (the same cluster used by Diesel repository tests).
2. After Diesel migrations complete, call `PostgresStorage::setup()` on the
   same database URL to create the Apalis job tables.
3. Create a `sqlx::PgPool` connected to the test database for the Apalis
   adapter.

Then add a behavioural suite at
`backend/tests/route_queue_apalis_bdd.rs` with a companion feature file at
`backend/tests/features/route_queue_apalis.feature`. Recommended scenarios:

- Happy path: enqueueing a plan persists it in the PostgreSQL job table.
- Happy path: enqueueing multiple distinct plans does not overwrite each other.
- Unhappy path: enqueueing when the database connection is invalid returns an
  unavailable error.
- Edge path: enqueueing the same plan twice results in two independent jobs
  (no deduplication at the adapter level).

Because this is a driven-adapter feature rather than an HTTP endpoint, the BDD
world can operate directly on the adapter and PostgreSQL harness instead of
booting the Actix server.

The BDD World struct should follow the pattern established by existing suites:

```rust
struct World {
    mode: TestMode,
    db_context: Option<DbContext>,
    enqueue_result: Option<Result<(), JobDispatchError>>,
}
```

Use `is_skipped(world)` gates for cluster setup failures, matching the pattern
in `backend/tests/user_state_startup_modes_bdd.rs`.

Stage D: document the architectural scope explicitly.

Update `docs/wildside-backend-architecture.md` to record:

- Roadmap item 5.2.1 introduces the Apalis-backed `RouteQueue` adapter with
  PostgreSQL storage, but does not yet enable route queue dispatch in runtime
  request flows or worker consumption.
- The queue adapter uses `sqlx::PgPool`, coexisting with the Diesel `bb8`
  pool used by repository adapters. Both pools connect to the same PostgreSQL
  instance.
- The adapter pattern follows the same `ConnectionProvider`-style abstraction
  used by the Redis cache adapter.
- The Apalis job tables are managed by `PostgresStorage::setup()`, not by
  Diesel migrations.
- The architecture document's earlier references to Redis as the job broker are
  superseded by the roadmap's specification of PostgreSQL.

Only after the implementation, tests, and full gates pass should
`docs/backend-roadmap.md` mark 5.2.1 done.

Stage E: replay the full repository gates.

Once focused queue tests are green, run the required repository gates with log
capture. `make test` is required even though the queue work uses its own SQLx
pool, because the repo's standard backend suites still rely on
`pg-embed-setup-unpriv` and the roadmap item cannot close without a clean
global gate run.

## Concrete steps

Run all commands from `/home/user/project`. Use `set -o pipefail` and `tee`
for every meaningful command so the exit code survives truncation and the log
is retained.

1. Capture the current stub-only baseline and confirm there is no Apalis
   adapter yet.

   ```bash
   set -o pipefail
   cargo test -p backend outbound::queue --lib 2>&1 \
     | tee /tmp/5-2-1-queue-baseline.out
   ```

   Expected pre-change signal:

   ```plaintext
   test ...stub_queue_enqueue_succeeds ... ok
   test result: ok. 1 passed; 0 failed;
   ```

2. Add Apalis dependencies to `backend/Cargo.toml` and verify the workspace
   compiles.

   ```bash
   set -o pipefail
   cargo check -p backend 2>&1 | tee /tmp/5-2-1-check-deps.out
   ```

3. Scaffold the adapter module, move the stub, and add focused `rstest`
   coverage. Then run the targeted queue tests until they pass.

   ```bash
   set -o pipefail
   cargo test -p backend outbound::queue --lib 2>&1 \
     | tee /tmp/5-2-1-queue-unit.out
   ```

   Expected green-state examples:

   ```plaintext
   test ...apalis_queue_enqueue_round_trips ... ok
   test ...apalis_queue_maps_provider_error_to_unavailable ... ok
   test ...apalis_queue_maps_serialization_failure_to_rejected ... ok
   test ...stub_queue_enqueue_succeeds ... ok
   test result: ok.
   ```

4. Add the PostgreSQL integration harness and BDD coverage, then run the
   focused scenarios.

   ```bash
   set -o pipefail
   cargo test -p backend --test route_queue_apalis_bdd \
     2>&1 | tee /tmp/5-2-1-queue-bdd.out
   ```

   Expected green-state examples:

   ```plaintext
   test route_queue_apalis_bdd::plan_is_persisted_in_queue ... ok
   test route_queue_apalis_bdd::invalid_connection_returns_unavailable ... ok
   test result: ok.
   ```

5. Update documentation after the focused suites are stable.

   ```bash
   set -o pipefail
   make markdownlint 2>&1 | tee /tmp/5-2-1-markdownlint.out
   set -o pipefail
   make nixie 2>&1 | tee /tmp/5-2-1-nixie.out
   ```

6. Run the required full gates before marking the roadmap item done.

   ```bash
   set -o pipefail
   make check-fmt 2>&1 | tee /tmp/5-2-1-check-fmt.out
   set -o pipefail
   make lint 2>&1 | tee /tmp/5-2-1-lint.out
   set -o pipefail
   make test 2>&1 | tee /tmp/5-2-1-test.out
   ```

7. Mark `docs/backend-roadmap.md` item 5.2.1 done only after the gate logs
   are clean, then append the log paths and outcome summary to this ExecPlan.

## Validation and acceptance

The implementation is done only when all of the following are true:

- Adapter behaviour:
  - `ApalisRouteQueue<P>` implements `RouteQueue<Plan = P>` using
    `apalis-postgres` `PostgresStorage`.
  - `enqueue` serializes the plan and pushes it to the PostgreSQL-backed Apalis
    job table.
  - serialization failures return `Err(JobDispatchError::Rejected { .. })`.
  - connection or storage failures return
    `Err(JobDispatchError::Unavailable { .. })`.
- Architectural boundaries:
  - domain code still knows only the `RouteQueue` trait and
    `JobDispatchError`;
  - no inbound adapter or domain service imports Apalis or SQLx types;
  - runtime request-path queue dispatch is not enabled as an accidental side
    effect;
  - the `TODO(#276)` markers in `route_submission` remain unchanged.
- Tests:
  - new `rstest` coverage passes for unit-level adapter behaviour;
  - new `rstest-bdd` coverage passes against a real embedded PostgreSQL
    instance;
  - existing Postgres-backed suites still pass through `make test`;
  - the existing `StubRouteQueue` tests still pass.
- Documentation:
  - `docs/wildside-backend-architecture.md` records the 5.2.1 scope decision
    and dual-pool architecture;
  - `docs/backend-roadmap.md` marks 5.2.1 done only after the gates pass.
- Gates:
  - `make check-fmt` passes;
  - `make lint` passes;
  - `make test` passes.

## Idempotence and recovery

All steps in this plan are designed to be re-runnable:

- `cargo check` and `cargo test` are idempotent.
- `PostgresStorage::setup()` is idempotent (creates tables if they do not
  exist).
- Diesel migrations are idempotent (tracked by migration version).
- `pg-embedded-setup-unpriv` provisions fresh template databases on each test
  run.
- If a step fails halfway, re-running it from the beginning is safe. No
  destructive or irreversible operations are performed.

If the embedded PostgreSQL cluster fails to start (a known environment issue in
this container), check `/dev/null` is a character device
(`ls -l /dev/null` should show `crw-rw-rw-`). If it is a regular file,
recreate it with `mknod -m 666 /dev/null c 1 3`.

## Interfaces and dependencies

### Dependencies to add to `backend/Cargo.toml`

```toml
# Queue adapter (Apalis with PostgreSQL)
apalis-postgres = "1.0.0-rc.6"

# SQLx for Apalis PostgreSQL pool (if not already present)
sqlx = { version = "0.8", features = ["postgres", "runtime-tokio"] }
```

The exact version of `apalis-postgres` should be verified against the latest
available release candidate at implementation time. If the version has advanced
beyond `1.0.0-rc.6`, evaluate the changelog for breaking changes and pin
accordingly.

### Module layout after implementation

```plaintext
backend/src/outbound/queue/
├── mod.rs                    (module header, re-exports)
├── stub_route_queue.rs       (moved from mod.rs, unchanged)
├── apalis_route_queue.rs     (new: adapter struct, provider trait, error mapping)
└── test_helpers.rs           (new, #[cfg(test)]: FakeQueueProvider)
```

### Key types and traits

In `backend/src/outbound/queue/apalis_route_queue.rs`, define:

```rust
/// Abstracts the queue storage backend for testability.
#[async_trait]
pub(crate) trait QueueProvider: Send + Sync {
    async fn push_job(&self, payload: Vec<u8>) -> Result<(), JobDispatchError>;
}

/// Apalis-backed provider using PostgreSQL storage.
#[derive(Debug, Clone)]
pub struct ApalisPostgresProvider { /* sqlx pool + storage */ }

/// Generic queue adapter parameterised over plan type and provider.
#[derive(Debug, Clone)]
pub struct GenericApalisRouteQueue<P, Q> { /* provider + PhantomData */ }

/// Production type alias.
pub type ApalisRouteQueue<P> =
    GenericApalisRouteQueue<P, ApalisPostgresProvider>;
```

In `backend/src/outbound/queue/test_helpers.rs`, define:

```rust
/// Fake provider that records pushed payloads for assertion.
pub(crate) struct FakeQueueProvider { /* internal state */ }

/// Fake provider that always returns an error.
pub(crate) struct FailingQueueProvider { /* error message */ }
```

### Test files

```plaintext
backend/tests/features/route_queue_apalis.feature   (Gherkin scenarios)
backend/tests/route_queue_apalis_bdd.rs              (step definitions)
```

## Approval / implementation gate

Status: AWAITING APPROVAL
