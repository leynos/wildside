# Architecture Decision Record (ADR) 002: Environment seam taxonomy

## Status

Accepted.

## Date

2026-09-06

## Context

Behaviour that depends on an environment variable used to acquire that value
wherever it was needed. `DefaultIdempotencyEnv` read `IDEMPOTENCY_TTL_HOURS`
inside the domain layer; the `ingest-osm` and `architecture-lint` binaries read
their configuration inside the policy that consumed it; and the shared embedded
PostgreSQL test support wrote `PG_PASSWORD` and `POSTGRESQL_RELEASES_URL` into
the process so a later library call would read them back.

Ambient access has two costs. It couples pure logic to a process-global, so
those code paths cannot be exercised without a stub process. And, because
`std::env::set_var` is unsound once other threads exist, any test that writes
the environment forces serialization: a global lock, a nextest group, or a
documented rule that the write must happen before a Tokio runtime is
constructed. That serialization is paid by the whole suite and it directly
reduces continuous-integration throughput and core utilization.

`backend/src/inbound/http/session_config.rs` already demonstrated the shape
worth generalizing: policy consumes a `SessionEnv` abstraction and `DefaultEnv`
is the process-backed adapter behind it.

## Decision

Prohibit ambient process-environment access with Clippy, and adopt a small
taxonomy of seams selected by how many call sites and variables a boundary has.

`clippy.toml` disallows `std::env::var`, `var_os`, `vars`, `vars_os`,
`set_var`, and `remove_var`, and `clippy::disallowed_methods` is denied so each
entry is a hard error under
`cargo clippy --workspace --all-targets --all-features`.

Choose the smallest seam that fits the boundary:

- **An explicit value**, for a one-off setting with a single consumer. The
  caller resolves the value and passes it in. `resolve_database_url` in
  `backend/src/bin/ingest_osm.rs` takes the `DATABASE_URL` value as an argument;
  `build_db_pool` in `backend/src/main.rs` takes `Option<String>`;
  `repair_password_state_serialized` in
  `backend/tests/support/stable_cluster_env.rs` takes the resolved password
  bytes and the resolved paths.
- **A narrow reader closure**, for a small boundary that reads more than one
  name or is exercised across several cases. The function takes
  `impl Fn(&str) -> Option<String>` rather than reading the process.
  `bind_addr` in `backend/src/main.rs` (`HOST` and `PORT`),
  `should_skip_test_cluster_with` in `backend/tests/support/cluster_skip.rs`,
  and `PasswordStatePaths::resolve` all use this shape.
- **An environment trait**, for a boundary mocked across many tests or expected
  to grow further inputs. `SessionEnv` in
  `backend/src/inbound/http/session_config.rs` and `IdempotencyEnv` in
  `backend/src/config/idempotency.rs` are the two such boundaries; each has a
  tiny process-backed adapter (`DefaultEnv`, `DefaultIdempotencyEnv`) that
  application composition injects.
- **Explicit child-process composition**, for subprocess tests. Build the
  child's environment with `Command::env_clear`, `Command::env`, and
  `Command::env_remove`. Mutating the test process to influence a child is not
  an accepted alternative.

A direct read may remain only at a genuine executable composition root — a
binary's `main` path or the one reader a test binary composes its support tree
with — and must carry an item-scoped
`#[expect(clippy::disallowed_methods, reason = "…")]` naming why the site is a
root. Use `expect`, never `allow`: the expectation goes unfulfilled, and
therefore warns, once the site is migrated, so the backlog removes itself
instead of rotting. The sanctioned roots today are `process_env` in
`backend/src/main.rs`, `database_url_from_process_env` in
`backend/src/bin/ingest_osm.rs`, `workspace_dir_override` in
`tools/architecture-lint/src/main.rs`, `process_env` in
`backend/tests/support/test_env.rs`, and the two adapter implementations named
above.

Nothing writes the process environment. Where a value has to reach a
third-party library that reads the environment itself, the *runner* composes
it: the `test-rust` Make recipe and the CI test and coverage steps supply
`PG_PASSWORD` and `POSTGRESQL_RELEASES_URL` to the test process, with `?=` and
per-step defaults so an existing value still wins.

## Consequences

- The shared embedded-cluster helper no longer has an ordering invariant. It
  performs no environment mutation, so setup paths may call it before or after
  constructing a Tokio runtime, and `STABLE_ENV_INIT` is gone along with the
  `OnceLock` that existed only to make one `set_var` safe.
- Test support that used a process-wide environment lock (`env_lock`) now
  passes stub readers or explicit values instead, so those suites no longer
  serialize on the environment. `env-lock` has been removed from the backend's
  development dependencies.
- Environment loading for idempotency moved out of `backend/src/domain` into
  `backend/src/config/idempotency.rs`. `IdempotencyConfig` is a pure value
  object with `from_ttl_hours`, and the domain no longer re-exports an
  environment-loading type.
- A new environment-dependent boundary picks a shape from the list above rather
  than inventing a fourth, and a reviewer can reject a trait for a
  single-variable, single-caller site or a bare closure for a boundary that
  several tests must mock.
- The policy is guarded by
  `backend/tests/environment_policy_contract.rs`, which fails if any of the six
  entries leaves `clippy.toml`, if a workspace member stops denying
  `clippy::disallowed_methods`, or if the `test-rust` recipe stops composing
  the embedded PostgreSQL settings.

## Alternatives considered

- **One shared environment service for the whole backend.** Rejected: it
  recreates the ambient coupling one layer down and gives single-variable sites
  a mock surface they do not need.
- **Keeping in-process `set_var` behind a lock.** Rejected: the lock is exactly
  the serialization this policy exists to remove, and `set_var` remains unsound
  once any other thread exists.
- **Passing explicit settings into the embedded PostgreSQL cluster
  constructor.** Preferred, but not available: `pg-embed-setup-unpriv` 0.5.x
  resolves its settings from the process environment through
  `bootstrap_for_tests`, with no explicit-settings variant. Runner-level
  composition is the closest available equivalent; an upstream
  explicit-settings API would let the wrapper drop even that.

## References

- Issue 464: enforce injected environment seams and prohibit ambient
  process-environment APIs.
- Issue 416: move `DefaultIdempotencyEnv`'s environment access out of the
  domain layer.
- Issue 434: replace environment-driven embedded PostgreSQL test support with
  explicit settings.
- "Environment seams" in [developers' guide](developers-guide.md).
- Netsuke's equivalent record, which this ADR mirrors:
  <https://github.com/leynos/netsuke/blob/main/docs/adr-008-environment-seam-taxonomy.md>
