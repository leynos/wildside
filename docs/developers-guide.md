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

## Front-end development

The Wildside Progressive Web Application (PWA) lives under `frontend-pwa/`.
The current checked-in package is intentionally smaller than the full v2a
target stack. Contributors must treat `frontend-pwa/package.json` as the
source of truth for installed packages, and use
`docs/v2a-front-end-stack.md` and `docs/frontend-roadmap.md` for target-stack
decisions that have not yet been implemented.

### Local setup

Use the repository Makefile entry points where possible. They keep Rust,
TypeScript, tokens, and documentation gates aligned:

```bash
make deps
make fmt
make lint
make test
```

The front-end package uses Bun-compatible workspace scripts, Vite `^7.3.2`,
React 19, React DOM 18, TanStack Query 5, Tailwind CSS `^3`, DaisyUI `^4`,
Zod 3, TypeScript 5, Vitest 3, and Orval 8. TanStack Router, Radix UI,
i18next, Fluent, MapLibre GL JS, Dexie, Tailwind CSS v4, and DaisyUI v5 are
target-stack items until a roadmap task adds them to `frontend-pwa/package.json`
and the lockfile.

### Build and preview workflow

Run front-end commands through workspace or Makefile targets unless debugging a
package-local failure:

```bash
make build-frontend
make test-frontend
make lint-frontend
make typecheck
```

Package-local commands remain useful while iterating in `frontend-pwa/`:

```bash
bun run dev
bun run build
bun run preview
bun run test
```

Token generation runs before front-end development, build, and preview scripts
through package hooks. The source token package is `packages/tokens/`; generated
outputs are consumed by `frontend-pwa/tailwind.config.js` and
`frontend-pwa/src/index.css`.

### Architectural patterns

Front-end implementation follows the roadmap phase order:

- Phase 0 reconciles source documents, design tokens, and imported v2a lint
  policy before feature work expands.
- Phase 1 establishes the build spine, provider layout, route metadata, data
  validation boundary, local-first storage boundary, and accessibility gates.
- Phase 2 delivers catalogue-led onboarding and discovery.
- Phase 3 delivers route generation, map-led quick generation, and active
  navigation surfaces.
- Phase 4 delivers installability, offline bundles, safety preferences, and
  completion summaries.
- Phase 5 evaluates deferred product extensions such as entitlement, richer
  pagination, native wrappers, community features, and reporting.

Feature code should be grouped by product capability rather than by technical
layer. Route modules should own page-level composition, feature modules should
own user-facing behaviour, shared query hooks should own server-state access,
and durable offline writes should pass through the outbox boundary described in
`docs/wildside-pwa-data-model.md`.

### Quality gates

Documentation-only front-end planning changes must pass:

```bash
make fmt
make markdownlint
```

Changes that touch Mermaid diagrams must also pass:

```bash
make nixie
```

Code changes under `frontend-pwa/` or `packages/tokens/` must pass the relevant
front-end gates plus the repository-wide commit gates:

```bash
make check-fmt
make lint
make test
```

When a phase introduces a new lint, accessibility, Playwright, or semantic CSS
gate, update this guide and the corresponding phase ExecPlan under
`docs/execplans/` in the same change.

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
domain-neutral primitives consumed only by adapters, not by the domain layer.
Domain modules must depend on domain ports and must not import
framework-specific types or `backend/src/models`. Test suites for these crates
follow a specific structure to keep individual files under the 400-line limit
and to validate both functional behaviour and documented invariants.

### File layout

Shared crate BDD suites live under `tests/` and scale by feature file count.
Each suite typically includes:

| File type                      | Purpose                                                        |
|--------------------------------|----------------------------------------------------------------|
| `common.rs`                    | Shared fixtures, world state, re-exports, and helpers          |
| `<crate>_bdd.rs`               | Core functional scenarios (one `#[scenario]` per feature file) |
| `<crate>_documentation_bdd.rs` | Scenarios verifying documented invariants (optional)           |

Additional `*_bdd.rs` files are created as needed when feature files are added.

For the pagination crate, this yields:

- `tests/common.rs` — `World` state struct, `FixtureKey`, and re-exports
- `tests/pagination_bdd.rs` — pagination foundation and direction-aware
  cursor scenarios
- `tests/pagination_documentation_bdd.rs` — documentation invariant
  scenarios (default limits, error variants, display strings)

### Fixture module pattern

The `common.rs` module contains:

- **Re-exports** of the crate's public API types, so step definitions import
  from `common::` rather than directly from the crate.
- **A `World` struct** deriving `ScenarioState` with `Slot<T>` fields for
  each piece of scenario state.
- **Domain-specific fixture types** (for example, a composite ordering key
  struct that mirrors the crate documentation examples).
- **Helper functions** that encapsulate multi-step setup shared across
  more than one step definition (for example, constructing a `PageParams`
  value from a raw limit). Helpers live in `common.rs` rather than a step
  binary so that both `<crate>_bdd.rs` and `<crate>_documentation_bdd.rs`
  can call them without duplicating logic.

Each BDD test binary declares `mod common;` and imports from it:

```rust
mod common;

use common::{Cursor, CursorError, Direction, FixtureKey, World};
```

When a step definition is needed by more than one test binary, define it
in both binaries rather than extracting it into the common module.
`rstest-bdd` step macros must appear in the same compilation unit as the
`#[scenario]` binding that references them.

When a setup action is needed by more than one step definition — across
either the same or different test binaries — extract it into a `pub fn`
in `common.rs`. Helper functions must not carry `#[given]`, `#[when]`, or
`#[then]` attributes; they are plain Rust functions called by step
definitions. Annotate helpers that use `.expect()` with
`#[expect(clippy::expect_used, reason = "BDD helpers use expect for clear failures")]`.
Prefer a helper over a duplicated step body as soon as the same
boilerplate appears in two or more places.

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
4. Build and validate crate documentation from the repository root:
   `RUSTDOCFLAGS="--cfg docsrs -D warnings" cargo doc -p <crate-name> --no-deps`.
   This ensures doc comments, examples, and links are valid.
5. Run doctests to verify code examples: `cargo test --doc -p <crate-name>`.
6. Create `tests/common.rs` with a `World` struct and crate re-exports.
7. Create one `*_bdd.rs` file per feature file under `tests/features/`.
8. If the crate includes substantial documentation with testable claims,
   add a `*_documentation_bdd.rs` file with invariant scenarios.
9. Ensure all test files stay under 400 lines; split by feature when
   needed.
10. Run the repository quality gates before committing: `make check-fmt`
    to verify formatting, `make lint` to verify linting, and `make test` to
    run the test suites. Documentation build and validation is performed
    separately via the `RUSTDOCFLAGS="--cfg docsrs -D warnings" cargo doc -p
    <crate-name> --no-deps` command described in step 4 above.

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

#### Production dependencies

- `bb8-redis` – Connection pooling for `redis-rs`
- `serde` / `serde_json` – Payload serialization

#### Test infrastructure

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

## Queue adapter testing

The backend includes an Apalis-backed `RouteQueue` adapter that persists jobs
to PostgreSQL storage via `apalis-postgres` / `PostgresStorage`. The adapter
implements the hexagonal `RouteQueue` port defined in the domain layer.

## Root-level script tests

Repository-level Node.js scripts under `scripts/` are tested with
[Vitest](https://vitest.dev/). The root `vitest.config.mjs` configures test
discovery:

- **Environment:** `node`
- **Test file pattern:** `scripts/**/*.test.mjs`

Run these tests with:

```sh
pnpm run test
```

Workspace package tests (frontend, etc.) are run separately:

```sh
pnpm run test:workspaces
```

`make test` runs both in sequence.

### Adding tests for a new script

1. Create a test file alongside the script: `scripts/<name>.test.mjs`.
2. Use `vi.mock` / `vi.resetModules` from Vitest to isolate each import.
3. If the script has CLI side-effects, gate them behind a direct-invocation
   guard (see "Programmatic API" under "Override parity check") so the module
   can be imported cleanly in tests.

## Override parity check

This repository pins certain security-sensitive dependencies in two separate
override blocks so they resolve correctly regardless of whether Bun or pnpm is
used for installation:

- `overrides` — top-level; consumed by Bun.
- `pnpm.overrides` — consumed by pnpm.

The script `scripts/check-overrides-parity.mjs` verifies that both blocks
contain identical values for every pinned dependency. It is run automatically
in Continuous Integration (CI) after the lockfile step and before dependency
installation.

### Running locally

```sh
node ./scripts/check-overrides-parity.mjs
```

A passing run prints:

```text
Override parity verified for basic-ftp, dompurify.
```

A failing run prints a per-dependency diff to stderr and exits with code `1`.

### Resolving failures

When the check fails, open `package.json` and ensure the version string in
`overrides.<package>` exactly matches the version string in
`pnpm.overrides.<package>`. Both entries must be present and identical.

### CI integration

The check runs as a step in `.github/workflows/ci.yml`:

```yaml
- run: node ./scripts/check-overrides-parity.mjs
```

It appears after `make lockfile` and before `make deps`. A failure here means
the two override blocks have drifted; update `package.json` and recommit.

### Programmatic API

`scripts/check-overrides-parity.mjs` exports two functions for use in tests or
other tooling:

- **`checkOverridesParity(packageJson)`** — accepts a parsed `package.json`
  object, writes diagnostics to `console.log` or `console.error`, and returns
  `0` on success or `1` on any mismatch or missing block.
- **`formatOverrideValue(value)`** — formats a single override value for
  human-readable diagnostics; returns `"<missing>"` for `undefined` and a
  JSON-stringified value otherwise.

Example import:

```js
import {
  checkOverridesParity,
  formatOverrideValue,
} from './scripts/check-overrides-parity.mjs';
```

The CLI entry point is protected by a direct-invocation guard so importing the
module does not trigger any file I/O or process side-effects:

```js
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  // only runs when invoked directly as `node ./scripts/check-overrides-parity.mjs`
}
```

## Operational references

- For local command quick reference and embedded PostgreSQL worker setup:
  [Wildside testing guide](wildside-testing-guide.md)
- For `rstest-bdd` API details and migration notes:
  - [rstest-bdd users' guide](rstest-bdd-users-guide.md)
  - [rstest-bdd v0.5.0 migration guide](rstest-bdd-v0-5-0-migration-guide.md)
- For embedded PostgreSQL API details and migration notes:
  - [pg-embed-setup-unpriv users' guide](pg-embed-setup-unpriv-users-guide.md)
  - [pg-embed-setup-unpriv v0.5.0 migration guide](pg-embed-setup-unpriv-v0-5-0-migration-guide.md)

### Apalis queue adapter boundaries

The hexagonal boundary is enforced via visibility:

| Component                            | Visibility                | Purpose                                    |
|--------------------------------------|---------------------------|--------------------------------------------|
| `ApalisRouteQueue<P>`                | `pub`                     | Public adapter for domain use              |
| `ApalisPostgresProvider`             | `pub`                     | Production `QueueProvider` implementation  |
| `GenericApalisRouteQueue<P, Q>`      | Internal; not re-exported | Generic adapter implementation             |
| `QueueProvider`                      | `pub(crate)`              | Test seam for provider abstraction         |
| `test_helpers::FakeQueueProvider`    | `pub(crate)` (test-only)  | In-memory test double                      |
| `test_helpers::FailingQueueProvider` | `pub(crate)` (test-only)  | Always-failing test double                 |
| `setup_apalis_storage`               | `pub` (test support)      | BDD harness for Apalis schema provisioning |

Domain code depends only on the `RouteQueue` port trait. The Apalis adapter
implements this port without exposing `apalis-postgres` or `sqlx` types in the
public API.

### Queue architecture and public API

Public production API:

- `ApalisRouteQueue<P>` – A type alias for the concrete Apalis-backed queue
  implementation. This is the supported public entry point for production code.
- `ApalisPostgresProvider` – The production `QueueProvider` implementation.
  Wraps `apalis_postgres::PostgresStorage<serde_json::Value>` and provisions
  the Apalis schema via `PostgresStorage::<(), (), ()>::setup`.

Implementation details within `outbound::queue`:

- `GenericApalisRouteQueue<P, Q>` – Declared `pub` inside the private
  `apalis_route_queue` module, but not re-exported at crate root. It
  parameterises the adapter over the queue provider type `Q` so tests can
  substitute doubles.
- `QueueProvider` – Declared `pub(crate)` inside the private
  `apalis_route_queue` module. Defines `async fn push_job(&self,
  payload: serde_json::Value) -> Result<(), JobDispatchError>` as the test
  seam; not part of the crate's supported public API.

### Queue build requirements

The queue adapter requires:

#### Production dependencies

- `apalis-core` – Core Apalis job-queue primitives
- `apalis-postgres` – PostgreSQL storage backend for Apalis
- `sqlx` (features: `postgres`, `runtime-tokio-rustls`) – Async PostgreSQL
  pool used by `ApalisPostgresProvider`
- `serde` / `serde_json` – Payload serialisation

#### Test infrastructure

- `pg-embedded-setup-unpriv` – Embedded PostgreSQL cluster for BDD tests
- No feature flags required; BDD tests are in the `tests/` integration
  harness and run unconditionally with `cargo test`

To run BDD tests locally:

```bash
# Run the Apalis BDD suite
cargo test -p backend --test route_queue_apalis_bdd
```

### Queue test infrastructure

**Unit tests** (run by default):

- Located in the `tests` module inside
  `backend/src/outbound/queue/apalis_route_queue.rs`
- Use `FakeQueueProvider` – an in-memory `QueueProvider` double that records
  all pushed payloads for assertion
- Use `FailingQueueProvider` – always returns
  `JobDispatchError::Unavailable`, for error-path testing
- Fast, deterministic, no external dependencies
- Run as part of the standard `cargo test` / `make test` gate

**BDD integration tests** (require embedded PostgreSQL):

- Feature file: `backend/tests/features/route_queue_apalis.feature`
- Step implementation: `backend/tests/route_queue_apalis_bdd.rs`
- Use `pg-embedded-setup-unpriv` to provision an embedded PostgreSQL cluster
- Apalis tables are created by `setup_apalis_storage` from
  `backend/tests/support/embedded_postgres.rs`, which calls
  `PostgresStorage::<(), (), ()>::setup(&pool)` after connecting a
  `sqlx::PgPool`
- Run as part of `cargo test --test route_queue_apalis_bdd` / `make test`

### `setup_apalis_storage` harness

BDD tests use `setup_apalis_storage` from
`backend/tests/support/embedded_postgres.rs`:

```rust
use backend::tests::support::embedded_postgres::setup_apalis_storage;

// Provision Apalis schema on an embedded PostgreSQL URL
let pool = runtime.block_on(setup_apalis_storage(&db_url))
    .expect("Apalis storage setup");
```

The helper:

- Connects a `sqlx::PgPool` to the supplied URL
- Calls `PostgresStorage::<(), (), ()>::setup(&pool)` to run Apalis
  schema migrations idempotently
- Returns the pool for use in subsequent BDD steps
