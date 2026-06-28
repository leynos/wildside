# Developers' guide

This guide is the canonical reference for day-to-day contributor workflows in
this repository. It explains how tests are structured, how behavioural tests
with `rstest-bdd` are written, and which quality gates must pass before commit.

Use this guide with the [Wildside testing guide](wildside-testing-guide.md).
The testing guide is an operations quick reference, while this document defines
strategy and usage conventions.

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
- `make audit`
- `make test`

## Local Kubernetes preview

Use the repository-local Kubernetes preview when validating the backend image
and Helm chart before handing values to Nile Valley. The preview workflow is
documented in
[Local Kubernetes preview and Nile Valley integration design](local-k8s-preview-design.md).

```bash
make local-k8s-up
make local-k8s-status
make local-k8s-logs
make local-k8s-down
```

The Makefile targets call `uv run scripts/local_k8s.py ...`. Keep helper logic
in `scripts/local_k8s/`, unit-test pure validation behaviour under
`scripts/local_k8s/unittests/`, and keep cluster creation idempotent. The
helper must fail before making changes when required tools for the selected
mode are missing.

Docker plus `k3d` remains the default local mode:

```bash
make local-k8s-up
```

Rootless Podman plus `kind` is the supported VM mode:

```bash
WILDSIDE_CONTAINER_ENGINE=podman WILDSIDE_K8S_PROVIDER=kind make local-k8s-up
```

The preferred configuration variables are `WILDSIDE_CONTAINER_ENGINE`,
`WILDSIDE_K8S_PROVIDER`, `WILDSIDE_K8S_CLUSTER`, and `WILDSIDE_K8S_PORT`.
`WILDSIDE_KIND_NODE_IMAGE` is an optional testing-only override for supported
Kubernetes upgrades; the default `kindest/node:v1.31.0` satisfies the chart's
kubeVersion range. `WILDSIDE_K3D_CLUSTER` and `WILDSIDE_K3D_PORT` remain legacy
aliases when the provider-neutral names are unset. In `kind` mode, use
`make local-k8s-status` to print the provider-specific
`kubectl port-forward` command before opening the preview port.

The helper also creates a `wildside-session-key` Secret when missing before
Helm installs the release, and reuses an existing key on later deploys.
`values.local.yaml` mounts that key at
`/var/run/secrets/wildside-session/session_key`, so local preview follows the release-mode
session-key path without committing secret material.

## Front-end development

The Wildside Progressive Web Application (PWA) lives under `frontend-pwa/`. The
current checked-in package is intentionally smaller than the full v2a target
stack. Contributors must treat `frontend-pwa/package.json` as the source of
truth for installed packages, and use `docs/v2a-front-end-stack.md` and
`docs/frontend-roadmap.md` for target-stack decisions that have not yet been
implemented.

Canonical front-end references:

- [Front-end source authority catalogue](frontend-source-authority-catalogue.md)
  identifies the authoritative source or reconciliation follow-up for each
  front-end platform, data, user experience, API, styling, accessibility,
  localization, and testing topic.
- [Front-end source contradictions catalogue](frontend-source-contradictions-catalogue.md)
  records concrete contradictions, duplicated requirements, and contract gaps.
  Pull requests that resolve a finding must update that row's `status` field.
- [v2a front-end stack](v2a-front-end-stack.md) documents the current package
  state and the target v2a stack boundary.
- [Wildside front-end roadmap](frontend-roadmap.md) is the implementation task
  catalogue and dependency map.
- [Wildside PWA design](wildside-pwa-design.md) documents the application
  shell, offline-first behaviour, routing, and platform requirements.
- [Wildside PWA data model](wildside-pwa-data-model.md) documents entity,
  outbox, offline bundle, and persistence contracts.
- [High-velocity accessibility-first component testing](high-velocity-accessibility-first-component-testing.md)
  documents the accessibility test strategy for component and browser tests.
- [Building accessible and responsive Progressive Web Applications](building-accessible-and-responsive-progressive-web-applications.md)
  documents PWA, responsive design, and Web Content Accessibility Guidelines
  (WCAG) expectations.
- [Semantic Tailwind with DaisyUI best practice](semantic-tailwind-with-daisyui-best-practice.md)
  and
  [Enforcing semantic Tailwind best practice](enforcing-semantic-tailwind-best-practice.md)
  document semantic styling and lint policy.
- [Frontend roadmap ExecPlan](execplans/frontend-roadmap.md) tracks overall
  roadmap execution, with phase-specific ExecPlans under `docs/execplans/`.

### Local setup

Use the repository Makefile entry points where possible. They keep Rust,
TypeScript, tokens, and documentation gates aligned:

```bash
make deps
make fmt
make lint
make audit
make test
```

`make audit` checks frontend and Rust dependencies. It expects Corepack to be
enabled so `pnpm` is available locally and in CI, and it requires
`cargo-audit` for the Rust dependency check.

The front-end package uses Bun-compatible workspace scripts, Vite `^7.3.5`,
React 19, React DOM 18, TanStack Query 5, Tailwind CSS `^3`, DaisyUI `^4`,
Zod 3, TypeScript 5, Vitest `^4.1.8`, and Orval 8. TanStack Router, Radix UI,
i18next, Fluent, MapLibre GL JS, Dexie, Tailwind CSS v4, and DaisyUI v5 are
target-stack items until a roadmap task adds them to `frontend-pwa/package.json`
and the lockfile.

### Build and preview workflow

Run front-end commands through workspace or Makefile targets unless debugging a
package-local failure:

```bash
make fe-build
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
through package hooks. The source token package is `packages/tokens/`;
generated outputs are consumed by `frontend-pwa/tailwind.config.js` and
`frontend-pwa/src/index.css`.

Makefile targets are the canonical local and Continuous Integration (CI) entry
points. Package-local Bun commands are allowed for focused iteration, but a
change is not ready to commit until the relevant Makefile gates pass.

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
make audit
make test
```

When a phase introduces a new lint, accessibility, Playwright, or semantic CSS
gate, update this guide and the corresponding phase ExecPlan under
`docs/execplans/` in the same change.

### Dependency audit helper modules

The JavaScript dependency-audit flow is split by responsibility:

- `security/audit-utils.js` is the orchestration surface used by package
  scripts. It exports `runAuditJson(auditIo?)` and
  `collectAdvisories(auditJson)`, and re-exports the lower-level package-data
  and reporting helpers.
- `security/audit-package-data.js` owns pure JSON parsing, `pnpm ls` package
  tree handling, installed-version maps, and npm bulk-advisory normalization.
  Its public helpers are `parseJsonOutput(payloadText, commandLabel, options?)`,
  `loadPackageTrees(auditIo, assertCompletedProcess)`,
  `buildVersionMap(packageTrees)`,
  `collectInstalledPackageVersions(auditIo, assertCompletedProcess)`, and
  `normalizeBulkAdvisories(bulkPayload)`.
- `security/audit-reporting.js` owns advisory partitioning and stderr output.
  Its public helpers are `partitionAdvisoriesById(advisories, allowedIds)` and
  `reportUnexpectedAdvisories(unexpected, heading, reportingIo = defaultReportingIo)`.
  The optional `reportingIo` adapter must expose an `error(...args)` method;
  pass a custom adapter in tests to capture output without writing to stderr.
  When omitted, `defaultReportingIo` delegates to `console.error`.
- `security/audit-exception-policy.js` owns exception-ledger date policy. Its
  public helper is `assertNoExpired(entries, currentDate?, policyIo?)`.
- `security/validate-audit.js` applies repository policy to the parsed audit
  results and the exception ledger.

Effectful audit helpers must receive external dependencies through the
`auditIo` adapter rather than reading process state directly. The default
adapter wraps `spawnSync`, `execFileSync`, `fetch`, timers, and `getEnv(name)`;
tests should inject an adapter with the same methods when they need to control
command results, registry configuration, network responses, or timeout
behaviour.

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

### Embedded PostgreSQL CI bootstrap stability

Continuous Integration (CI) warms the `pg-embed-setup-unpriv` binary cache with
`scripts/warm-pg-embedded-cache.sh` before running `cargo nextest`. Keep this
warm-up step before `Rust tests`; it turns PostgreSQL binary acquisition into a
short, explicit CI step instead of letting the first integration test perform a
cold download inside `postgresql_embedded::setup()`.

The CI cache step must include both binary-cache locations used by the two
embedded PostgreSQL layers:

- `~/.theseus/postgresql` for `postgresql_embedded` runtime installations.
- `~/.cache/pg-embedded/binaries` for `pg-embed-setup-unpriv` release archives.

Do not co-locate those paths inside the Cargo registry/cache archive. Cargo
dependency updates and `Cargo.lock` churn otherwise evict the PostgreSQL binary
cache and force a fresh download during unrelated test changes.

The warm-up step pins:

- `POSTGRESQL_VERSION="=16.10.0"` so archive resolution does not need wildcard
  release discovery.
- `POSTGRESQL_RELEASES_URL=https://github.com/theseus-rs/postgresql-binaries`
  so the binary source cannot drift when crate defaults change.
- `GITHUB_TOKEN` so GitHub release requests use the Actions token and avoid
  anonymous rate limits.

Keep PostgreSQL-backed nextest binaries in the `pg-embedded` test group in
`.config/nextest.toml`, and keep that group serialized. First-use cluster
bootstrap is process-local and expensive; serial execution avoids concurrent
setup attempts competing for the same warmed cache, filesystem paths, and
worker process.

If CI reports `error decoding response body`, treat it as a likely download
stall or timeout from `reqwest` rather than as JSON/body corruption. Check the
`Cache PostgreSQL embedded binaries` and
`Warm PostgreSQL embedded binary cache` steps first, then verify that the
`Rust tests` step is still exporting `PG_EMBEDDED_WORKER`, `GITHUB_TOKEN`, and
`NEXTEST_TEST_THREADS=1`.

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
5. Run all commit gates before commit:
   `make check-fmt`, `make lint`, `make audit`, and `make test`.

When migrating existing suites, prefer incremental edits that preserve scenario
intent and avoid broad rewrites that obscure regressions.

When validating generated `rstest-bdd` integration binaries, prefer running the
binary directly:

```bash
cargo test -p backend --test startup_mode_composition_bdd -- --nocapture
```

Name filters can miss generated scenario functions or select only support
module tests. Use filters only after confirming the generated scenario names
match the filter text.

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
| ------------------------------ | -------------------------------------------------------------- |
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
  more than one step definition (for example, constructing a `PageParams` value
  from a raw limit). Helpers live in `common.rs` rather than a step binary so
  that both `<crate>_bdd.rs` and `<crate>_documentation_bdd.rs` can call them
  without duplicating logic.

Each BDD test binary declares `mod common;` and imports from it:

```rust
mod common;

use common::{Cursor, CursorError, Direction, FixtureKey, World};
```

When a step definition is needed by more than one test binary, define it in
both binaries rather than extracting it into the common module. `rstest-bdd`
step macros must appear in the same compilation unit as the `#[scenario]`
binding that references them.

When a setup action is needed by more than one step definition — across either
the same or different test binaries — extract it into a `pub fn` in
`common.rs`. Helper functions must not carry `#[given]`, `#[when]`, or
`#[then]` attributes; they are plain Rust functions called by step definitions.
Annotate helpers that use `.expect()` with
`#[expect(clippy::expect_used, reason = "BDD helpers use expect for clear failures")]`.
Prefer a helper over a duplicated step body as soon as the same boilerplate
appears in two or more places.

### Hexagonal consumption rules

Shared workspace crates sit below the adapter layer and must remain
transport-agnostic:

- **Inbound adapters** consume shared crate types for deserialization and
  response wrapping (for example, deserializing `PageParams` from query strings
  and wrapping results in `Paginated<T>`).
- **Outbound adapters** consume shared crate types for query construction
  (for example, using `Cursor` keys for keyset filtering in Diesel queries).
- **Domain code** does not depend on shared crate types directly. Ports
  define their own parameter and return types; adapters convert at the boundary.
- **Error mapping** is performed by inbound adapters, not by the shared
  crate. The crate documents recommended HTTP status codes and envelope `code`
  values, but the adapter layer owns the final mapping.
- **Pagination-aware repository errors** are modelled as semantic port errors
  rather than opaque query strings. For users pagination,
  `UserPersistenceError::Pagination` wraps `UserPaginationError`, allowing
  repository-originated invalid cursor and unsupported direction failures to
  map to HTTP `400` while connection failures remain `503` and unexpected query
  failures remain redacted `500` responses.
- **BDD cursor fixtures** that exercise invalid opaque cursor payloads should
  use static base64url tokens via `concat!` when the request helper expects a
  `'static` path. This keeps the behaviour explicit without adding test-only
  dependencies to an adapter crate.

### Documentation invariant testing

When a shared crate includes crate-level documentation that makes specific
claims (default values, error behaviour, normalization rules), create a
dedicated `*_documentation_bdd.rs` test file with scenarios that verify those
claims at runtime. This ensures documentation and implementation remain in sync
as a gating requirement.

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
    to verify formatting, `make lint` to verify linting, `make audit` to
    verify dependency audits, and `make test` to run the test suites.
    Documentation build and validation is performed separately via the
    `RUSTDOCFLAGS="--cfg docsrs -D warnings" cargo doc -p <crate-name>
    --no-deps` command described in step 4 above.

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

Related domain helpers:

- `RouteCacheKey::for_route_request(...)` derives canonical
  `route:v1:<digest>` keys by normalizing route payloads before hashing.
- `RouteCacheKeyDerivationError` reports `Hash` and `Validation` failures from
  key derivation.

#### Test infrastructure

- `pg-embedded-setup-unpriv` – Embedded PostgreSQL cluster for BDD tests
- No feature flags required; BDD tests are in the `tests/` integration
  harness and run unconditionally with `cargo test`

To run BDD tests locally:

```bash
# Ensure pg-embedded-setup-unpriv is available
pg-embedded-setup-unpriv --help

# Start the embedded PG cluster
pg-embedded-setup-unpriv start

# Run Redis route-cache BDD tests
cargo test -p backend --test route_cache_redis_bdd
```

### RedisTestServer harness

Integration tests use `RedisTestServer` from
`backend/src/test_support/redis.rs`:

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

#### Test infrastructure

- `pg-embedded-setup-unpriv` – Embedded PostgreSQL cluster for BDD tests
- No feature flags required; BDD tests are in the `tests/` integration
  harness and run unconditionally with `cargo test`

To run BDD tests locally:

```bash
# Ensure pg-embedded-setup-unpriv is available
pg-embedded-setup-unpriv --help

# Start the embedded PG cluster (adjust data directory as needed)
pg-embedded-setup-unpriv start

# Run integration BDD tests
cargo test -p backend --test '*'
```

### Adapter boundaries

The hexagonal boundary is enforced via visibility:

| Component                      | Visibility                | Purpose                              |
| ------------------------------ | ------------------------- | ------------------------------------ |
| `RedisRouteCache<P>`           | `pub`                     | Public adapter for domain use        |
| `GenericRedisRouteCache<P, C>` | Internal; not re-exported | Generic adapter implementation       |
| `ConnectionProvider`           | Internal; not re-exported | Test seam for connection abstraction |
| `RedisPoolProvider`            | Internal; not re-exported | Production `ConnectionProvider` impl |
| `test_helpers::FakeProvider`   | `pub` (test-only)         | In-memory test double                |
| `RedisTestServer`              | `pub` (test-support)      | Live server harness                  |

Domain code depends only on the `RouteCache` port trait. The Redis adapter
implements this port without exposing `bb8-redis` types in the public API.

### Queue / Apalis dependencies

The queue adapter requires:

- `apalis-core` – Core Apalis job-queue primitives
- `apalis-postgres` – PostgreSQL storage backend for Apalis
- `sqlx` (features: `postgres`, `runtime-tokio-rustls`) – Async PostgreSQL
  pool used by `ApalisPostgresProvider`
- `serde` / `serde_json` – Payload serialization

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

### Python script tests

Scripts that are better tested with Python use pytest. The test file lives
alongside the script and is named `scripts/<name>_test.py` (note: underscore,
not dot, so pytest can import it as a module).

**Test file pattern:** `scripts/**/*_test.py`

**Run all Python script tests:**

```sh
pytest scripts/
```

**Run a specific test file:**

```sh
pytest scripts/warm_pg_embedded_cache_test.py -v
```

Refer to `docs/scripting-standards.md` for Python tooling conventions,
dependency declarations (PEP 723 inline metadata), and style guidance.

### Adding tests for a new script

1. Create a test file alongside the script: `scripts/<name>.test.mjs`.
2. Use `vi.mock` / `vi.resetModules` from Vitest to isolate each import.
3. If the script has CLI side-effects, gate them behind a direct-invocation
   guard (see "Programmatic API" under "Override policy check") so the module
   can be imported cleanly in tests.

## UX audit helpers

`scripts/audit-ux-state-graph.mjs` supports front-end source catalogue work by
checking a JSON UX state graph against routes cited in a Markdown sitemap. It
is review tooling only: it must not change runtime behaviour, generate
artefacts, or become a source of product requirements.

### Input contract

The graph file must be JSON with:

- `states` — an array of objects with a string `id` and optional string
  `route`.
- `transitions` — an array of objects with string `from` and `to` endpoint
  ids.
- `initialState` — optional string id for the state that is allowed to have no
  inbound transitions.

The sitemap file is Markdown. The helper extracts backtick-quoted absolute
routes such as `` `/cards` `` and compares state routes with exact, wildcard,
and hash-stripped matching.

### Output contract

A successful run writes one deterministic line per state:

```text
<state-id> in=<count> out=<count> route=<route-or-NONE> [ORPHAN]
```

The `route` field defaults to `NONE` when a state omits a route. A state is
marked `[ORPHAN]` when it is not the initial state and has no inbound
transitions, has no outbound transitions, or names a route absent from the
sitemap.

### Running locally

```sh
bun run scripts/audit-ux-state-graph.mjs \
  --graph docs/wildside-ux-state-graph-v0.1.json \
  --sitemap docs/sitemap.md
```

A run prints one line per state:

```text
<state-id> in=<count> out=<count> route=<route-or-NONE> [ORPHAN]
```

Input or parsing errors are printed to stderr and exit with code `1`.

## Override policy check

This repository pins certain security-sensitive dependencies with
`pnpm.overrides`. Keep these install-time dependency patches scoped to pnpm.
Do not add a top-level `overrides` block: npm consumes that block for ordinary
commands such as `npx`, and rejects overrides that conflict with direct
dependency ranges.

Bun audit exceptions are handled by `security/run-bun-audit.js`, which turns
non-expired entries in `security/audit-exceptions.json` into explicit
`bun audit --ignore=<GHSA>` flags. This keeps Bun audit policy visible without
changing npm's dependency resolution surface.

The script `scripts/check-overrides-policy.mjs` verifies that
`pnpm.overrides` is present and that top-level overrides are absent. It is run
automatically in Continuous Integration (CI) after the lockfile step and before
dependency installation.

### Running locally

```sh
node ./scripts/check-overrides-policy.mjs
```

A passing run prints:

```text
pnpm override policy verified for basic-ftp, dompurify, ip-address, uuid.
```

A failing run prints a policy diagnostic to stderr and exits with code `1`.

### Resolving failures

When the check fails, open `package.json` and remove any top-level
`overrides` entries. Keep dependency patches under `pnpm.overrides`; for Bun
audit output, add a time-bound entry to `security/audit-exceptions.json` and
let `pnpm run audit:bun` pass the corresponding advisory ID to Bun.

### CI integration

The check runs as a step in `.github/workflows/ci.yml`:

```yaml
- run: node ./scripts/check-overrides-policy.mjs
```

It appears after `make lockfile` and before `make deps`. A failure here means
an npm-visible override has been added or the pnpm override policy has been
removed; update `package.json` and recommit.

### Programmatic API

`scripts/check-overrides-policy.mjs` exports three functions for use in tests or
other tooling:

- **`checkOverridesPolicy(packageJson)`** — accepts a parsed `package.json`
  object and returns a structured report with `ok`, `pnpmOverridesToCheck`,
  `rootOverrides`, and `reason` fields. It is a query helper and must not write
  to stdout or stderr.
- **`formatOverrideValue(value)`** — formats a single override value for
  human-readable diagnostics; returns `"<missing>"` for `undefined` and a
  JSON-stringified value otherwise.
- **`reportOverridesPolicy(report, outputIo?)`** — writes the structured report
  to a console-like adapter and returns process exit code `0` or `1`. The CLI
  entrypoint is the only production caller that uses the default `console`
  adapter.

Example import:

```js
import {
  checkOverridesParity,
  formatOverrideValue,
  reportOverridesParity,
} from './scripts/check-overrides-policy.mjs';
```

The CLI entry point is protected by a direct-invocation guard so importing the
module does not trigger any file I/O or process side-effects:

```js
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  // only runs when invoked directly as `node ./scripts/check-overrides-policy.mjs`
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

| Component                            | Visibility               | Purpose                                    |
| ------------------------------------ | ------------------------ | ------------------------------------------ |
| `ApalisRouteQueue<P>`                | `pub`                    | Public adapter for domain use              |
| `ApalisPostgresProvider`             | `pub`                    | Production `QueueProvider` implementation  |
| `GenericApalisRouteQueue<P, Q>`      | `pub`                    | Generic adapter and BDD harness seam       |
| `QueueProvider`                      | `pub(crate)`             | Test seam for provider abstraction         |
| `test_helpers::FakeQueueProvider`    | `pub(crate)` (test-only) | In-memory test double                      |
| `test_helpers::FailingQueueProvider` | `pub(crate)` (test-only) | Always-failing test double                 |
| `setup_apalis_storage`               | `pub` (test support)     | BDD harness for Apalis schema provisioning |

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

- `GenericApalisRouteQueue<P, Q>` – Re-exported beside the production alias
  because the BDD harness constructs the adapter with a test provider. It can
  parameterize the adapter over the queue provider type `Q`, so tests can
  substitute doubles, while production code should prefer `ApalisRouteQueue<P>`.
- `QueueProvider` – Declared `pub(crate)` inside the private
  `apalis_route_queue` module. Defines
  `async fn push_job(&self, payload: serde_json::Value) -> Result<(), JobDispatchError>`
  as the test seam; not part of the crate's supported public API.

Queue observability:

- `RouteQueueMetrics` – The domain-owned metrics port used by queue adapters
  to record enqueue outcomes without depending on Prometheus or process-global
  state.
- `RouteQueueOutcome` – The bounded outcome label type for queue metrics. It
  currently exposes only `success` and `failure`, keeping metric cardinality
  predictable.
- `NoOpRouteQueueMetrics` – The no-op implementation used by tests and
  metrics-disabled builds. Prefer it whenever a test is not asserting metrics
  behaviour.
- `PrometheusRouteQueueMetrics` – The Prometheus adapter in
  `outbound::metrics`. It accepts a `prometheus::Registry` at construction
  time, allowing tests to use isolated registries while production passes the
  default registry.
- Concurrency coverage – Queue tests spawn concurrent enqueue tasks through a
  shared `Arc<GenericApalisRouteQueue<_, _>>`, while metrics tests spawn
  concurrent `PrometheusRouteQueueMetrics` initialization attempts against an
  isolated registry. This keeps shared state and duplicate-registration
  behaviour covered without using process-global Prometheus state.
- `route_queue_enqueue_total{outcome=success|failure}` – A feature-gated
  Prometheus counter for enqueue throughput and outcome.
- `route_queue_enqueue_latency_seconds{outcome=success|failure}` – A
  feature-gated Prometheus histogram for end-to-end queue enqueue latency in
  seconds.
- `tracing` – The adapter emits `debug` on enqueue success and `warn` on
  failure points (serialization, push, and setup failures), including latency
  and the adapter outcome.

### Queue build requirements

The queue adapter requires:

#### Production dependencies

- `apalis-core` – Core Apalis job-queue primitives
- `apalis-postgres` – PostgreSQL storage backend for Apalis
- `sqlx` (features: `postgres`, `runtime-tokio-rustls`) – Async PostgreSQL
  pool used by `ApalisPostgresProvider`
- `serde` / `serde_json` – Payload serialization

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

## Spelling policy

The `make spelling` gate enforces en-GB-oxendict spelling across tracked text.
It runs Typos 1.48.0 and a phrase checker that rejects the hyphenated form in
favour of `handwritten`. `make markdownlint` depends on the same spelling gate.

The tracked `typos.toml` is generated from the shared Oxford dictionary and the
repository-specific `typos.local.toml` overlay. The generator is the focused
`typos-config-builder` command pinned to commit
`d6da92f02240a79a945c835f69bdd08a888da1d0`. It refreshes the untracked
`.typos-oxendict-base.toml` cache only when the authority is newer than the
local copy; `.typos-oxendict-base.json` records refresh metadata.

Use `make spelling-config-write` after changing `typos.local.toml`, and use
`make spelling-config` to check deterministic output. Never edit `typos.toml`
directly. Keep repository exceptions narrow: preserve external APIs, formal
names, wire values and immutable fixtures without adding ordinary bare-word
exceptions.

The standalone phrase helper and its tests use Python 3.14 at runtime,
Pathspec 1.1.1 and a Python 3.13 Ruff compatibility target. Continuous
integration installs Nixie 1.1.0 and Merman CLI 0.7.0 before validating the
repository's Mermaid diagrams with `make nixie`.
