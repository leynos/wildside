# Mutex-backed global state locking for test helpers in pg-embed-setup-unpriv

## Summary

`pg-embed-setup-unpriv` (the crate) and `pg_embedded_setup_unpriv` (the Rust
library surface) provide helpers for provisioning and running an embedded
PostgreSQL instance for automated test suites.

Some of these helpers mutate process-global state, primarily via environment
variables (and, depending on integration and downstream usage, sometimes the
current working directory). This state is shared across all tests within the
same process. When tests are executed concurrently, the current behaviour can
produce data races at the behavioural level (even though Rust remains memory
safe), leading to intermittent failures.

This proposal adds a small, internal, mutex-backed locking mechanism for
process-global state mutations performed by test helpers (notably
`bootstrap_for_tests()` and `TestCluster`). The lock is held for the duration
of the guard returned by these helpers, ensuring that tests cannot
concurrently mutate or observe partially configured process state.

The intention is to make `pg-embed-setup-unpriv` safe-by-default for typical
Rust test runners, including `cargo test`, `cargo nextest`, and doctests,
without requiring every consumer to implement its own global locking.

## Problem statement

### What fails today

Rust tests frequently run in parallel. Within a single process, tests share
several forms of global mutable state, including:

- The process environment (for example, `PGPASSFILE`, `HOME`, and XDG Base
  Directory (XDG) directories).
- The current working directory.
- Any global caches keyed by environment values.

`pg_embedded_setup_unpriv` already provides test-oriented functionality that
sets and relies on environment variables as part of its contract. If multiple
tests construct clusters at the same time, each test can:

- overwrite environment variables the other test relies on,
- observe partially applied settings,
- restore variables while another test still needs them.

The resulting failures are typically intermittent and scheduler dependent.
They are hard to reproduce, and they often manifest as test flakiness.

### Why this is not a user error

In most Rust test environments, parallelism is the default, and consumers
should not need to be experts in process-global state to use a "test helper"
crate correctly.

If a crate:

- mutates global state, and
- expects callers to rely on that state during a test,

then it is the crate's responsibility to make those mutations safe for the
expected execution model.

## Use case

A typical integration test wants a ready-to-use PostgreSQL instance and a
consistent set of environment variables for the duration of the test.

For example:

- A `TestCluster` guard is created.
- The test uses `cluster.settings()` to derive a connection URL.
- The test uses existing application code that reads environment variables
  configured by the cluster.
- The guard is dropped and the cluster shuts down.

This should be reliable even when:

- the suite runs with multiple test threads,
- the suite is executed by `cargo nextest`, and
- doctests run concurrently with other tests.

## Current workaround

Downstream consumers often implement ad hoc locking and scoping themselves.
Common patterns include:

- creating a global `Mutex<()>` to serialise cluster bootstrap,
- setting environment variables before constructing the cluster, and
- restoring environment variables in `Drop`.

Some projects use the `env-lock` crate to avoid re-implementing this logic.
Other projects disable test parallelism (for example, via
`cargo test -- --test-threads=1`) to avoid races.

These workarounds are effective, but they are:

- duplicated across consumers,
- easy to get subtly wrong, and
- incompatible with the crate's promise that `TestCluster` is Resource
  Acquisition Is Initialization (RAII) friendly because the crate is still
  relying on global mutable state.

## Proposed solution

### Design goals

- Make test helpers safe under parallel test execution in a single process.
- Keep the locking and restoration logic within `pg-embed-setup-unpriv`.
- Avoid introducing a new public dependency as part of the default build.
- Preserve current behaviour and environment variable semantics.

### Non-goals

- Provide cross-process locking.
  - If multiple processes run tests concurrently, they should already be using
    unique runtime and data directories.
- Make unrelated application-level environment access "safe".
  - The goal is to ensure `pg-embed-setup-unpriv` does not cause unsafe
    interleavings for its own test helper flows.

### Implementation sketch

Introduce a small internal module that owns process-global locks.

- `static ENV_MUTEX: Mutex<()> = Mutex::new(());`
- Optionally, if the crate mutates the working directory:
  `static CURRENT_DIR_MUTEX: Mutex<()> = Mutex::new(());`

Update test helpers so they acquire the environment lock before mutating any
variables and hold it for the lifetime of the returned guard.

There are two reasonable implementation shapes.

#### Option A: internal lock held by test guards (recommended)

- `TestCluster` stores a `MutexGuard<'static, ()>` so the environment remains
  locked for the entire lifetime of the cluster.
- Any environment restoration performed by `Drop` happens while the lock is
  held.

This matches the intuitive contract:

- If a test holds a live cluster guard, the process environment required for
  that cluster is stable.

#### Option B: expose explicit lock helpers (future extension)

Expose additional helpers such as:

- `lock_process_env(...) -> ProcessEnvGuard`

This provides flexibility, but it expands the public API surface and requires
callers to be more deliberate.

Option B is still useful when supporting consumers who:

- do not want to use `TestCluster`, but still want safe environment mutation.

### API changes

This proposal recommends Option A as the default, with the smallest public API
impact.

- No breaking changes to existing public APIs.
- Locking is internal and always enabled for test helpers.

### Behavioural details

- Lock acquisition should tolerate poisoning.
  - If a test panics while holding the lock, the lock should still be usable by
    subsequent tests.
  - The crate can recover the inner mutex value on poison, which is a common
    approach for test utilities.
- Lock scope:
  - The lock is acquired before setting any environment variables.
  - The lock is released after restoring environment variables.
- Performance:
  - Tests that use `TestCluster` become serialised within a process.
  - This is acceptable because these tests are already heavyweight
    (process-spawning, filesystem-heavy, and frequently network bound).

## Example usage

No usage changes are required for the default path.

```rust,no_run
use pg_embedded_setup_unpriv::TestCluster;

fn integration_test() -> Result<(), String> {
    let cluster = TestCluster::new().map_err(|err| format!("{err:?}"))?;
    let url = cluster.settings().url("app_db");

    // Run queries against `url`.

    drop(cluster);
    Ok(())
}
```

Internally, `TestCluster::new()` would ensure that all environment mutations it
performs are protected by a process-global lock, preventing concurrent cluster
bootstraps from interfering with one another.

## Alternatives considered

### 1. Document "run tests with one thread"

For example, `cargo test -- --test-threads=1`.

- Pros: no code changes.
- Cons: shifts responsibility to every consumer, reduces test throughput, and
  is easy to forget in Continuous Integration (CI).

### 2. Require callers to pass configuration explicitly (no env vars)

Provide a purely functional API where callers pass all configuration in
arguments.

- Pros: avoids global state entirely.
- Cons: not immediately compatible with existing integrations that expect
  environment variables, and it would likely be a breaking change.

This is still a worthwhile long-term direction, but a mutex-based fix is a
pragmatic step that can be delivered without breaking consumers.

### 3. Depend on env-lock

Use `env-lock` internally.

- Pros: avoids implementing locking and restoration logic.
- Cons: introduces a new dependency and ties the crate's design to another
  crate's API and release cadence.

Given the small scope of the required functionality (a mutex and scoped
restoration), an internal implementation is likely preferable.

### 4. Use per-variable locks

Lock only the variables `pg-embed-setup-unpriv` mutates.

- Pros: more concurrency.
- Cons: significantly more complexity, and still error-prone because many tests
  read multiple variables as a bundle.

A single process-global lock is simpler and matches the nature of the shared
resource.

## Why this belongs in pg-embed-setup-unpriv

This functionality belongs in `pg-embed-setup-unpriv` because:

- The crate already performs process-global mutations as part of its test
  helpers' contract (for example, setting `PGPASSFILE` and XDG directories).
- These mutations are not merely local implementation details; they are
  observable behaviour relied upon by consumers.
- Consumers cannot safely use the crate in the default Rust test execution
  model without either:
  - re-implementing locking themselves, or
  - disabling parallelism.

By internalising the lock, the crate provides:

- a stable, deterministic interface for tests,
- a single, audited implementation of the tricky "global state" concerns, and
- less friction and fewer foot-guns for downstream users.

## Backwards compatibility and migration

- No migration is required.
- Existing consumers benefit immediately.
- The only observable behavioural change is that concurrent uses of
  `TestCluster` (and other test helpers that mutate env) become serialised in a
  single process.

## Testing plan

- Add unit tests that:
  - spawn two threads that attempt to acquire the lock and mutate a sentinel
    environment variable, and
  - assert that the second thread observes either the pre-lock or post-drop
    environment state, never an intermediate state.
- Add behavioural tests (if available in the crate) that:
  - start and stop clusters in parallel threads, and
  - assert deterministic restoration of environment variables.
- Ensure doctests remain stable under `cargo test`.

## Open questions

- Should the lock be enabled for all code paths, or only test helpers?
- Should the crate also lock mutations to the current working directory if it
  performs any such mutations today?
- Should an explicit `lock_process_env` helper be exported, or kept internal?
