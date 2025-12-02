# Wildside testing guide

This guide summarises how to run the test suites locally and which toggles to
adjust when running without elevated privileges.

## Core commands

- `make fmt` – formats Rust and JS/TS sources.
- `make lint` – runs Clippy and Biome lint checks.
- `make check-fmt` – verifies formatting without writing changes.
- `make test` – executes Rust nextest suites, workspace JS/TS tests, and
  Python script tests. Honour the notes below before running.

## Embedded Postgres worker path

`make test` builds and installs the `pg_worker` helper used by
`pg_embedded_setup_unpriv`. By default it writes to `/var/tmp/pg_worker`, which
may fail on systems where you lack write access.

- Set `PG_WORKER_PATH` to a user-writable location when running locally, for
  example:

  ```bash
  PG_WORKER_PATH=/tmp/pg_worker make test
  ```

- The Makefile forwards this value to `PG_EMBEDDED_WORKER`, keeping behaviour
  consistent between the helper and the test runner while preserving the
  default CI path.

## Troubleshooting

- Permission denied during `prepare-pg-worker`: re-run with `PG_WORKER_PATH` as
  shown above.
- Doctor-style errors from doctests: ensure `TraceId` imports use the public
  re-export (`use backend::TraceId;`).
