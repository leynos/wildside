# Wildside testing guide

This guide summarizes how to run the test suites locally and which toggles to
adjust when running without elevated privileges.

For behavioural test strategy, fixture conventions, and contributor workflow
rules, read the [developers guide](developers-guide.md) first. This file
focuses on command execution and local environment setup.

## Core commands

- `make fmt` – formats Rust and JS/TS sources.
- `make lint` – runs Clippy and Biome lint checks.
- `make check-fmt` – verifies formatting without writing changes.
- `make test` – executes Rust nextest suites, workspace JS/TS tests, and
  Python script tests. Honour the notes below before running.

## Behavioural test stack

Rust behavioural suites use `rstest-bdd` v0.5.0 with strict compile-time
validation through `rstest-bdd-macros`. This keeps Gherkin scenarios aligned
with local step definitions at build time.

## Embedded Postgres worker path

`make test` now uses the `pg_worker` helper shipped by
`pg-embed-setup-unpriv` (imported as `pg_embedded_setup_unpriv` in Rust).

- If `pg_worker` is already installed and on `PATH`, the Makefile copies that
  binary to `PG_WORKER_PATH`.
- Otherwise, the Makefile installs `pg-embed-setup-unpriv`'s `pg_worker` into
  a cache under `target/pg-worker-root/` and then copies it to
  `PG_WORKER_PATH`.
- By default, `PG_WORKER_PATH` is `target/pg_worker`. Override it when you need
  a different writable destination, for example:

  ```bash
  PG_WORKER_PATH=/tmp/pg_worker make test
  ```

- The Makefile forwards `PG_WORKER_PATH` to `PG_EMBEDDED_WORKER`, keeping the
  test runner and the helper aligned while still allowing local overrides.

## Embedded Postgres test strategy

Backend integration tests use the shared cluster helpers from
`pg-embed-setup-unpriv` v0.5.0. A single embedded PostgreSQL instance is
started per test process, and each test receives a temporary database cloned
from a migration-backed template. This keeps per-test setup fast while
preserving database-level isolation.

The test harness sets `PG_TEST_BACKEND=postgresql_embedded` when not already
provided. This matches v0.5.0 strict backend validation and keeps local and CI
behaviour explicit.

If full cluster-level isolation is required (for example, to change
server-wide settings), switch the test to the per-test `TestCluster` helper
instead of the shared cluster path.

To force the template to rebuild locally, delete the workspace cache under
`target/pg-embed/shared-*` and re-run the relevant tests.

## Troubleshooting

- Permission denied during `prepare-pg-worker`: re-run with `PG_WORKER_PATH` as
  shown above, or ensure the destination directory is writable.
- Doctor-style errors from doctests: ensure `TraceId` imports use the public
  re-export (`use backend::TraceId;`).
