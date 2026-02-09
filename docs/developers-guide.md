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

## Operational references

- For local command quick reference and embedded PostgreSQL worker setup:
  [Wildside testing guide](wildside-testing-guide.md)
- For `rstest-bdd` API details and migration notes:
  - [rstest-bdd users' guide](rstest-bdd-users-guide.md)
  - [rstest-bdd v0.5.0 migration guide](rstest-bdd-v0-5-0-migration-guide.md)
- For embedded PostgreSQL API details and migration notes:
  - [pg-embed-setup-unpriv users' guide](pg-embed-setup-unpriv-users-guide.md)
  - [pg-embed-setup-unpriv v0.5.0 migration guide](pg-embed-setup-unpriv-v0-5-0-migration-guide.md)
