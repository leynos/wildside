# Developers' guide

This guide is the canonical reference for day-to-day contributor workflows in
this repository. It explains how tests are structured, how behavioural tests
with `rstest-bdd` are written, and which quality gates must pass before
commit.

Use this guide with the [Wildside testing
guide](wildside-testing-guide.md). The testing guide is an operations
quick reference, while this document defines strategy and usage conventions.

## Testing strategy

The test suite combines unit, integration, and behavioural tests:

- Unit tests cover pure domain logic and edge-case handling in small scopes.
- Integration tests verify adapter behaviour against real infrastructure, such
  as embedded PostgreSQL.
- Behavioural tests describe user-visible flows in Gherkin features and bind
  them to Rust step definitions.

All suites run through the same quality gateways:

- `make check-fmt`
- `make lint`
- `make test`

## Embedded PostgreSQL integration tests

Backend integration and behavioural suites that require PostgreSQL use the
shared helpers under `backend/tests/support/`:

- `pg_embed::shared_cluster()` provisions one embedded PostgreSQL cluster per
  test process.
- `embedded_postgres::provision_template_database()` creates a per-test
  temporary database cloned from a migration-backed template.
- `cluster_skip::handle_cluster_setup_failure()` is the single policy point for
  converting setup failures into explicit `SKIP-TEST-CLUSTER` outcomes.

This strategy is the default because it keeps test runtime low while preserving
database-level isolation. Only use direct per-test cluster construction when a
suite truly needs cluster-level isolation (for example, server-wide settings or
lifecycle-specific assertions).

### v0.5.0 migration usage rules

When migrating to `pg-embed-setup-unpriv` `v0.5.0`, apply these conventions so
test usage remains coherent:

- Keep `PG_TEST_BACKEND` limited to supported values (`postgresql_embedded` or
  unset/empty). Treat unsupported values as intentional skip/failure signals,
  not implicit fallback.
- Prefer the handle/guard split APIs for shared or send-bound fixtures where
  appropriate (`ClusterHandle` with explicit lifecycle ownership).
- Continue using template-database cloning (`ensure_template_database` and
  `temporary_database_from_template`) for per-test isolation rather than
  rerunning migrations per test.
- Use `CleanupMode::None` only for explicit debugging sessions where retained
  files are required; keep deterministic cleanup defaults for normal runs.

## Rust behavioural tests with `rstest-bdd` v0.5.0

### Dependency contract

Behavioural suites pin `rstest-bdd` and macros to `0.5.0` in dev-dependencies.
The macros use strict compile-time validation:

```toml
[dev-dependencies]
rstest-bdd = "0.5.0"
rstest-bdd-macros = { version = "0.5.0", features = ["strict-compile-time-validation"] }
```

This enforces step and scenario consistency during compilation and prevents
feature files drifting away from local step definitions.

### Scenario binding conventions

Keep scenario fixture parameter names aligned with step fixture names. In this
repository, many suites use `world` as the shared fixture key.

Use explicit no-op bindings in scenario functions:

```rust
#[scenario(path = "tests/features/example.feature")]
fn example_scenario(world: ExampleWorld) {
    drop(world);
}
```

`drop(world);` is intentional. It preserves fixture-key compatibility for step
injection while keeping warning gates green under `RUSTFLAGS="-D warnings"`.

### State isolation model

Scenario state is isolated by default:

- Prefer per-scenario fixtures and `ScenarioState` data structures.
- Use `Slot<T>` from `rstest-bdd` to hold optional or late-bound values within
  one scenario.
- Use `rstest`'s `#[once]` fixture attribute only for expensive, effectively
  read-only infrastructure.
- Do not rely on scenario execution order or cross-scenario mutable state.

### Async step guidance

`rstest-bdd` v0.5.0 supports async step definitions. In this repository:

- Prefer synchronous step functions when the harness already owns a runtime and
  deterministic execution is more important than style changes.
- Use async steps when they materially reduce adapters/wrappers and do not
  create nested runtime issues.
- If a synchronous scenario must run async-only work, rely on `rstest-bdd`'s
  per-step Tokio current-thread fallback for async-only steps in synchronous
  scenarios rather than creating nested Tokio runtimes manually.

### Where behavioural tests live

- Backend scenarios and steps:
  - Feature files: `backend/tests/features/`
  - Scenario bindings: `backend/tests/*_bdd.rs`
  - Shared BDD helpers: `backend/tests/support/`
- Example-data scenarios and steps:
  - Feature files: `crates/example-data/tests/features/`
  - Scenario bindings: `crates/example-data/tests/*_bdd.rs`
- Shared workspace crate scenarios and steps:
  - Feature files: `backend/crates/<crate>/tests/features/`
  - Scenario bindings: `backend/crates/<crate>/tests/*_bdd.rs`
  - Shared fixtures: `backend/crates/<crate>/tests/common.rs`

## Adding or changing behavioural tests

When adding a new behaviour:

1. Add or update the Gherkin scenario under the correct `tests/features/`
   directory.
2. Add or update Rust step definitions with `#[given]`, `#[when]`, and
   `#[then]`.
3. Add or update the scenario binding function with `#[scenario(...)]`.
4. Keep fixture naming consistent across scenario binding and step functions.
5. Run all three gates before commit:
   `make check-fmt`, `make lint`, and `make test`.

When migrating existing suites, prefer incremental edits that preserve scenario
intent and avoid broad rewrites that obscure regressions.

## Shared workspace crate testing

Shared workspace crates (such as `backend/crates/pagination`) provide
domain-neutral primitives consumed by multiple layers of the hexagonal
architecture. Their test suites follow a specific structure to keep
individual files under the 400-line limit and to validate both functional
behaviour and documented invariants.

### File layout

Shared crate BDD suites split into three files under `tests/`:

| File                           | Purpose                                                        |
|--------------------------------|----------------------------------------------------------------|
| `common.rs`                    | Shared fixtures, world state, re-exports, and helpers          |
| `<crate>_bdd.rs`               | Core functional scenarios (one `#[scenario]` per feature file) |
| `<crate>_documentation_bdd.rs` | Scenarios verifying documented invariants                      |

For the pagination crate, this yields:

- `tests/common.rs` — `World` state struct, `FixtureKey`, and re-exports
- `tests/pagination_bdd.rs` — pagination foundation and direction-aware
  cursor scenarios
- `tests/pagination_documentation_bdd.rs` — documentation invariant
  scenarios (default limits, error variants, display strings)

### Fixture module pattern

The `common.rs` module contains:

- **Re-exports** of the crate's public API types so step definitions import
  from `common::` rather than directly from the crate.
- **A `World` struct** deriving `ScenarioState` with `Slot<T>` fields for
  each piece of scenario state.
- **Domain-specific fixture types** (for example, a composite ordering key
  struct that mirrors the crate documentation examples).

Each BDD test binary declares `mod common;` and imports from it:

```rust
mod common;

use common::{Cursor, CursorError, Direction, FixtureKey, World};
```

When a step definition is needed by more than one test binary, define it
in both binaries rather than extracting it into the common module.
`rstest-bdd` step macros must appear in the same compilation unit as the
`#[scenario]` binding that references them.

### Hexagonal consumption rules

Shared workspace crates sit below the adapter layer and must remain
transport-agnostic:

- **Inbound adapters** consume shared crate types for deserialization and
  response wrapping (for example, deserializing `PageParams` from query
  strings and wrapping results in `Paginated<T>`).
- **Outbound adapters** consume shared crate types for query construction
  (for example, using `Cursor` keys for keyset filtering in Diesel
  queries).
- **Domain code** does not depend on shared crate types directly. Ports
  define their own parameter and return types; adapters convert at the
  boundary.
- **Error mapping** is performed by inbound adapters, not by the shared
  crate. The crate documents recommended HTTP status codes and envelope
  `code` values, but the adapter layer owns the final mapping.

### Documentation invariant testing

When a shared crate includes crate-level documentation that makes specific
claims (default values, error behaviour, normalization rules), create a
dedicated `*_documentation_bdd.rs` test file with scenarios that verify
those claims at runtime. This ensures documentation and implementation
remain in sync as a gating requirement.

### Integration guidance for new crates

When adding a new shared workspace crate:

1. Add the crate path to `[workspace].members` in the root `Cargo.toml`.
2. Run `make fmt` to synchronize the auto-discovered members list.
3. Verify the crate compiles and tests pass: `cargo test -p <crate-name>`.
4. Create `tests/common.rs` with a `World` struct and crate re-exports.
5. Create one `*_bdd.rs` file per feature file under `tests/features/`.
6. If the crate includes substantial documentation with testable claims,
   add a `*_documentation_bdd.rs` file with invariant scenarios.
7. Ensure all test files stay under 400 lines; split by feature when
   needed.
8. Run the repository quality gates before committing: `make check-fmt`,
   `make lint`, and `make test` to verify formatting, linting, and all
   tests pass.

## Redis cache adapter testing

The backend includes a Redis-backed `RouteCache` adapter for caching route
computation results. The adapter uses `bb8-redis` for connection pooling and
implements the hexagonal `RouteCache` port defined in the domain layer.

### Architecture and public API

Public production API:

- `RedisRouteCache<P>` – A type alias for the concrete Redis-backed cache
  implementation. This is the supported public entry point for production code.

Implementation details within `outbound::cache`:

- `GenericRedisRouteCache<P, C>` – Declared `pub` inside the private
  `redis_route_cache` module, but not re-exported from `outbound::cache`, so it
  is not part of the crate's supported public API. It parameterizes the adapter
  over the connection provider type `C` so tests can substitute doubles.
- `ConnectionProvider` – Declared `pub` inside the private
  `redis_route_cache` module, but not re-exported from `outbound::cache`, so it
  remains an implementation detail rather than a supported public extension
  point. Production uses `RedisPoolProvider`, while tests substitute
  `FakeProvider`.
- `RedisPoolProvider` – Declared `pub` inside the private `redis_route_cache`
  module, but not re-exported from `outbound::cache`. It backs
  `RedisRouteCache<P>` with `bb8-redis` pooling and remains an internal
  implementation detail.

### Test infrastructure

The Redis adapter test suite uses a dual-mode approach:

**Mock-based unit tests** (run by default):

- Located in `backend/src/outbound/cache/tests/mock_tests.rs`
- Use `FakeProvider` – an in-memory `ConnectionProvider` double
- Fast, deterministic, no external dependencies
- Run as part of the standard `cargo test` / `make test` gate

**Live Redis integration tests** (opt-in):

- Located in `backend/src/outbound/cache/tests/live_tests.rs`
- Require a `redis-server` binary on `PATH`
- Marked with `#[ignore = "requires redis-server binary..."]`
- Run explicitly with: `cargo test -- --ignored`

### RedisTestServer harness

Integration tests use `RedisTestServer` from `backend/src/test_support/redis.rs`:

```rust
use backend::test_support::redis::RedisTestServer;

// Start a temporary redis-server process on an ephemeral port
let server = RedisTestServer::start().await?;

// Get a connection URL
let url = server.redis_url(); // e.g., "redis://127.0.0.1:12345/"

// Build a bb8-redis pool (requires test-support feature)
let pool = server.pool().await?;

// Or seed raw bytes for error-path testing
server.seed_raw_bytes("key", vec![0xff, 0xfe]).await?;
```

The harness spawns a real `redis-server` process with:

- Ephemeral port binding (`127.0.0.1:0`)
- Disabled persistence (`--save "" --appendonly no`)
- Temporary working directory (automatically cleaned up on drop)
- Process termination on drop (via `Drop` impl)

### Build requirements

The cache adapter requires:

**Production dependencies:**

- `bb8-redis` – Connection pooling for `redis-rs`
- `serde` / `serde_json` – Payload serialization

**Test infrastructure:**

- `test-support` feature flag – Enables `RedisRouteCache::new()` constructor
  and `RedisTestServer::pool()` for test injection
- `redis-server` binary – Required for live integration tests (not for unit
tests using `FakeProvider`)

To run live Redis tests locally:

```bash
# Ensure redis-server is available
which redis-server

# Run ignored tests explicitly
cargo test -p backend --lib outbound::cache -- --ignored
```

### Adapter boundaries

The hexagonal boundary is enforced via visibility:

| Component                      | Visibility                  | Purpose                              |
|--------------------------------|-----------------------------|--------------------------------------|
| `RedisRouteCache<P>`           | `pub`                       | Public adapter for domain use        |
| `GenericRedisRouteCache<P, C>` | Internal; not re-exported   | Generic adapter implementation       |
| `ConnectionProvider`           | Internal; not re-exported   | Test seam for connection abstraction |
| `RedisPoolProvider`            | Internal; not re-exported   | Production `ConnectionProvider` impl |
| `test_helpers::FakeProvider`   | `pub` (test-only)           | In-memory test double                |
| `RedisTestServer`              | `pub` (test-support)        | Live server harness                  |

Domain code depends only on the `RouteCache` port trait. The Redis adapter
implements this port without exposing `bb8-redis` types in the public API.

## Operational references

- For local command quick reference and embedded PostgreSQL worker setup:
  [Wildside testing guide](wildside-testing-guide.md)
- For `rstest-bdd` API details and migration notes:
  - [rstest-bdd users' guide](rstest-bdd-users-guide.md)
  - [rstest-bdd v0.5.0 migration guide](rstest-bdd-v0-5-0-migration-guide.md)
- For embedded PostgreSQL API details and migration notes:
  - [pg-embed-setup-unpriv users' guide](pg-embed-setup-unpriv-users-guide.md)
  - [pg-embed-setup-unpriv v0.5.0 migration guide](pg-embed-setup-unpriv-v0-5-0-migration-guide.md)
