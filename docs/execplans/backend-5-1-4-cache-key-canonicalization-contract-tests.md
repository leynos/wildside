# Implement route cache key canonicalization contract tests

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: BLOCKED

## Purpose / big picture

Roadmap item 5.1.4 closes the gap between the architecture document and the
shipping backend by proving that route cache keys are canonicalized before
they reach Redis. After this work, semantically equivalent route requests
reuse the same cache entry even if their theme arrays are reordered, their
object keys arrive in a different order, or their coordinates differ only
outside the documented five-decimal precision.

The user-visible outcome is indirect but important: cache hit rates improve for
repeated route requests, and the canonicalization contract becomes testable in
both fast unit tests and a Redis-backed behavioural test.

## Constraints

- Keep canonicalization in the domain-owned cache-key seam. Do not move this
  logic into `backend/src/outbound/cache/redis_route_cache.rs`.
- Preserve the existing `RouteCache` adapter contract. The Redis adapter should
  continue to consume `RouteCacheKey` values rather than deriving them.
- Do not add new production dependencies.
- Keep new or modified Rust modules below the repository's 400-line limit.
- Update `docs/backend-roadmap.md` and any architecture prose touched by this
  work so the documentation matches the implemented design.

## Tolerances (exception triggers)

- Scope: if the change requires modifying more than 8 files or more than 300
  net lines of Rust outside tests and docs, stop and review the design.
- Interface: if a new public port trait or inbound HTTP wire contract is
  required, stop and escalate.
- Dependencies: if any new crate is needed, stop and escalate.
- Iterations: if two focused test-and-fix loops still leave the new coverage
  failing, stop and reassess before widening scope.
- Infrastructure: if Redis-backed behavioural tests cannot run because the
  environment lacks `redis-server`, accept a skipped behavioural scenario but
  still complete the unit-contract coverage and note the limitation here.

## Risks

- Risk: The current repository only stores opaque JSON route payloads, so the
  canonicalization seam could overfit a guessed request shape.
  Severity: medium
  Likelihood: medium
  Mitigation: Normalize only the documented semantic rules: sorted theme arrays,
  rounded coordinate fields, stable object ordering, and `route:v1:<sha256>`
  formatting.

- Risk: Extending the existing Redis BDD file would breach the 400-line module
  limit.
  Severity: medium
  Likelihood: high
  Mitigation: Add a new focused BDD file and feature instead of extending
  `backend/tests/route_cache_redis_bdd.rs`.

- Risk: The environment may serialize Cargo builds behind an existing lock.
  Severity: low
  Likelihood: medium
  Mitigation: Wait for the lock, then run the full Makefile gates with `tee`
  logs before closing the turn.

## Implementation outline

1. Extend `backend/src/domain/ports/cache_key.rs` with a constructor that
   derives canonical `RouteCacheKey` values from route-request JSON.
2. Normalize the documented semantic equivalences before hashing:
   sorted theme arrays, rounded coordinate fields, and stable object-key
   ordering.
3. Reuse the domain SHA-256 payload hashing helper so the final key format
   remains `route:v1:<sha256>`.
4. Add fast unit coverage in `backend/src/domain/ports/cache_key.rs` for
   namespace shape, sorted themes, rounded coordinates, and material
   differences.
5. Add Redis-backed behavioural coverage in a new integration test so storing a
   plan under one canonical key can be read back via an equivalent request's
   canonical key.
6. Update the roadmap and architecture prose, then run all required code and
   Markdown gates.

## Validation

The change is only complete when all of the following succeed:

```plaintext
make fmt
make markdownlint
make nixie
make check-fmt
make lint
make test
```

If Redis-backed behavioural coverage is skipped because `redis-server` is not
available, the test output must clearly show the skip reason and the unit tests
must still pass.

## Progress

- [x] 2026-04-24 00:00 UTC: Reconstructed context from notes, roadmap, and the
  route-cache/domain modules after context compaction.
- [x] 2026-04-24 00:00 UTC: Confirmed the ExecPlan file was missing from the
  checkout and restored it as a living document.
- [x] 2026-04-24 00:00 UTC: Added a domain-owned canonical route cache key
  constructor plus focused unit coverage in `backend/src/domain/ports/cache_key.rs`.
- [x] 2026-04-24 00:00 UTC: Added a dedicated Redis-backed behavioural test and
  feature file for equivalent-request cache hits without extending the existing
  near-limit BDD file.
- [x] 2026-04-24 00:00 UTC: `make fmt`, `make markdownlint`,
  `make nixie`, `make check-fmt`, and `make lint` completed successfully.
- [ ] 2026-04-24 00:00 UTC: `make test` is still required to close the task.
  The gate is currently blocked by `make prepare-pg-worker`, which repeatedly
  terminates while compiling the external `pg-embed-setup-unpriv` helper
  binary in this container.
- [ ] 2026-04-24 00:00 UTC: Record final outcomes and durable project notes.

## Surprises & Discoveries

- The referenced ExecPlan path was absent from this checkout even though the
  prior session summary described it. Restoring the plan is part of this task.
- `backend/tests/route_cache_redis_bdd.rs` already sits at 387 lines, so even a
  small scenario addition would violate the repository's 400-line file limit.
- The repository already has a reusable SHA-256 canonical JSON helper in
  `backend/src/domain/idempotency/payload.rs`, which keeps the new cache-key
  seam small and consistent.
- The roadmap checkbox cannot be marked complete yet because the project rules
  require all automated checks, including `make test`, to pass before closure.

## Decision Log

- Decision: Implement canonicalization as `RouteCacheKey::for_route_request`
  inside `backend/src/domain/ports/cache_key.rs`.
  Rationale: The architecture document says cache-key canonicalization belongs
  to the service/domain seam, not to the Redis adapter.

- Decision: Reuse the existing domain hash helper instead of introducing a new
  hashing path.
  Rationale: This avoids duplicate SHA-256 logic and keeps canonical JSON
  hashing consistent across the codebase.

- Decision: Split behavioural coverage into a new integration test file.
  Rationale: The existing Redis BDD file is already too close to the 400-line
  ceiling to extend safely.

- Decision: Leave roadmap item 5.1.4 unchecked for now.
  Rationale: The implementation and most gates are complete, but the
  repository's change policy requires `make test` to pass before a roadmap item
  can be closed.

## Outcomes & Retrospective

Implementation is complete and the following validations passed:
`cargo test -p backend route_request_key --lib`,
`cargo test -p backend --test route_cache_key_canonicalization_bdd -- --nocapture`,
`make fmt`, `make markdownlint`, `make nixie`, `make check-fmt`, and
`make lint`.

The remaining blocker is environmental rather than product-code specific:
`make test` cannot complete because `make prepare-pg-worker` is terminated
mid-build while compiling `pg-embed-setup-unpriv v0.5.0`. Once that helper
binary is available on `PATH` or in `target/pg_worker`, the full test gate
should be rerun before marking roadmap item 5.1.4 complete.
