# Phase 0 architecture guardrails (HTTP + WebSocket adapter tests)

This ExecPlan is a living document. The sections `Progress`,
`Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective` must
be kept up to date as work proceeds.

There is no `PLANS.md` in this repository, so this ExecPlan is the sole
execution reference.

## Purpose / Big Picture

Add integration tests that drive the real Actix HTTP and WebSocket handlers
over real network sockets while substituting domain-facing ports with
deterministic test doubles. This provides a guardrail that:

- inbound adapters remain side effect free (no database/network I/O inside the
  handler/actor),
- domain logic stays framework-agnostic (handlers coordinate; domain types and
  ports do the work),
- future changes must preserve the dependency direction and seam boundaries.

Observable success:

- Running `make check-fmt`, `make lint`, and `make test` succeeds.
- New tests exist that:
  - start an Actix server bound to `127.0.0.1:0` (ephemeral port),
  - exercise `POST /api/v1/login`, `GET /api/v1/users`, and `/ws` upgrade,
  - use mocked *domain ports* (use-case traits) to control responses and record
    calls,
  - cover both happy and unhappy paths (including edge cases like malformed
    WebSocket JSON).
- `docs/wildside-backend-architecture.md` records the design decisions taken
  for the port seams introduced for these tests.
- `docs/backend-roadmap.md` marks “Architecture guardrails: integration tests”
  as done.

## Progress

- [x] (2025-12-15) Create inbound “use-case port” traits for HTTP + WS.
- [x] (2025-12-15) Refactor HTTP handlers to use injected ports via `web::Data`.
- [x] (2025-12-15) Refactor WebSocket actor/handler to use injected port.
- [x] (2025-12-15) Add unit tests (`rstest`) for new port implementations.
- [x] (2025-12-15) Add behavioural tests (`rstest-bdd` v0.1.0) covering:
  - HTTP happy path (login + list users) and unhappy path (invalid credentials),
  - WS happy path (valid payload) and unhappy path (malformed JSON, invalid
    payload). (Display-name rejection mapping remains covered by existing
    WebSocket session tests.)
- [x] (2025-12-15) Ensure embedded Postgres remains usable locally via
  `pg-embedded-setup-unpriv` (guardrails rely on existing Postgres fixtures).
- [x] (2025-12-15) Update `docs/wildside-backend-architecture.md` decision log.
- [x] (2025-12-15) Mark roadmap item done in `docs/backend-roadmap.md`.
- [x] (2025-12-15) Run `make check-fmt`, `make lint`, `make test` and
  fix failures. Note: `make test` required a longer timeout (≈3 minutes of
  embedded Postgres contract tests) than the default 300 seconds.
- [x] (2025-12-15) Remove stale `http/auth.rs` reference from the inbound
  adapter Mermaid diagram in `docs/wildside-backend-architecture.md`.

## Surprises & Discoveries

- Observation: `rstest-bdd-macros` does not support async step definitions.
  Evidence: existing pattern in `backend/tests/diesel_user_repository.rs`.
- Observation: Actix Web (and `actix_web::rt::spawn`) uses Tokio
  `spawn_local`, which panics unless run inside a `tokio::task::LocalSet`.
  Evidence: initial guardrail tests failed with
  `spawn_local called from outside of a task::LocalSet or LocalRuntime`.

## Decision Log

- Decision: Introduce “use-case ports” (driving ports) as traits in
  `backend/src/domain/ports/` and inject them into inbound adapters via
  `actix_web::web::Data`. Rationale: Allows adapter integration tests to supply
  deterministic test doubles and assert call boundaries without requiring
  outbound I/O. Date/Author: 2025-12-14 / Codex CLI.

## Outcomes & Retrospective

- Added driving-port traits and fixture implementations so handlers/actors can
  depend on injected use-cases instead of constructing domain logic directly.
- Added adapter integration tests that:
  - start an Actix server on `127.0.0.1:0`,
  - use deterministic port test doubles,
  - exercise HTTP and WS flows via `awc`.
- Recorded the key decision in the backend architecture document and marked
  the roadmap entry as done.

## Context and Orientation

Key locations (repository-relative):

- `backend/src/inbound/http/*`: HTTP handlers (Actix).
- `backend/src/inbound/ws/*`: WebSocket entry + actor.
- `backend/src/domain/*`: framework-agnostic domain types, services, and ports.
- `backend/tests/*`: integration tests (compiled as separate crates).
- `docs/backend-roadmap.md`: delivery checklist (must be updated on completion).
- `docs/wildside-backend-architecture.md`: architecture decisions and patterns.

Terminology (plain-language):

- *Port*: a trait that defines an interaction boundary. In this plan we use:
  - driven ports (domain → infrastructure), e.g. `UserRepository`,
  - driving ports (inbound adapter → domain use-case), e.g. `LoginService`.
- *Adapter*: framework-facing code at the edge (HTTP handlers, WS actor).

## Plan of Work

1. Add driving-port traits in `backend/src/domain/ports/`:
   - `LoginService`: validates credentials and returns a `UserId` or domain
     `Error`.
   - `UsersQuery`: returns the visible users list for a given authenticated
     `UserId`.
   - `UserOnboarding`: maps a WebSocket display name request into a domain
     `UserEvent` (CPU-only).

2. Provide “fixture” implementations inside the domain that preserve current
   runtime behaviour:
   - `FixtureLoginService` matches the existing `admin` / `password` behaviour.
   - `FixtureUsersQuery` returns the current hard-coded user list.
   - `UserOnboardingService` already exists; implement the new trait for it.

3. Refactor inbound adapters to depend on those traits:
   - HTTP: `login` and `list_users` accept `web::Data<HttpState>` holding the
     injected ports.
   - WS: `ws_entry` accepts `web::Data<WsState>`, and `WsSession` is
     constructed with an injected `Arc<dyn UserOnboarding>`.

4. Add tests:
   - Unit tests (`rstest`) for fixture services and edge cases (domain-level).
   - Behaviour tests (`rstest-bdd`) that:
     - start an Actix server on an ephemeral port,
     - install mocked ports that record calls,
     - exercise HTTP and WS via real clients (`awc`),
     - assert both outputs and call boundaries.
   - Postgres embedded remains covered by existing tests (do not regress).

5. Update documentation:
   - Add an explicit design decision to
     `docs/wildside-backend-architecture.md`.
   - Mark the guardrails item “done” in `docs/backend-roadmap.md`.

6. Run the quality gates using `make` targets and fix failures.

## Concrete Steps

Run these commands from the repository root:

1. Format (write changes):

    make fmt

2. Verify formatting (no writes):

    make check-fmt

3. Lint (Clippy, rustdoc, architecture lint, Biome, infra linters):

    make lint

4. Test (Rust nextest + JS tests + scripts tests):

    make test

If the output is long, capture it to a log (recommended):

    set -o pipefail
    make test 2>&1 | tee /tmp/wildside-make-test.log

## Validation and Acceptance

Acceptance criteria:

1. Adapter guardrails tests:
   - HTTP tests demonstrate that `POST /api/v1/login`:
     - calls the injected `LoginService`,
     - sets a session cookie on success,
     - returns `401` on invalid credentials.
   - HTTP tests demonstrate that `GET /api/v1/users`:
     - requires an authenticated session,
     - calls the injected `UsersQuery` when authenticated.
   - WS tests demonstrate that `/ws`:
     - calls the injected `UserOnboarding` on valid JSON payloads,
     - closes with a policy error on malformed JSON without calling the port.

2. Quality gates:
   - `make check-fmt` passes.
   - `make lint` passes.
   - `make test` passes.

## Idempotence and Recovery

- All steps are safe to re-run.
- If embedded Postgres is unavailable on the machine, tests that require it
  should skip with a clear message (existing pattern: `SKIP-TEST-CLUSTER:`).
- If a long-running command times out, re-run it with a longer timeout and
  keep the log file for inspection.

## Artifacts and Notes

- Key test harness pattern for async + `rstest-bdd`:
  see `backend/tests/diesel_user_repository.rs` for a context holding a Tokio
  runtime and synchronous step definitions that `block_on` async operations.

## Interfaces and Dependencies

New/updated interfaces (final state):

- In `backend/src/domain/ports/login_service.rs`, define:

    #[async_trait::async_trait]
    pub trait LoginService: Send + Sync {
        async fn authenticate(
            &self,
            credentials: &crate::domain::LoginCredentials,
        ) -> Result<crate::domain::UserId, crate::domain::Error>;
    }

- In `backend/src/domain/ports/users_query.rs`, define:

    #[async_trait::async_trait]
    pub trait UsersQuery: Send + Sync {
        async fn list_users(
            &self,
            authenticated_user: &crate::domain::UserId,
        ) -> Result<Vec<crate::domain::User>, crate::domain::Error>;
    }

- In `backend/src/domain/ports/user_onboarding.rs`, define:

    pub trait UserOnboarding: Send + Sync {
        fn register(
            &self,
            trace_id: crate::TraceId,
            display_name: String,
        ) -> crate::domain::UserEvent;
    }

Revision note (required when editing an ExecPlan):

- Initial version authored on 2025-12-15 to implement Phase 0 guardrails tests
  described in `docs/backend-roadmap.md`.

- Updated on 2025-12-15 after implementation to record:
  - the LocalSet requirement for Actix integration tests,
  - completed progress items,
  - the remaining work (running full repo quality gates).

- Updated on 2025-12-15 to remove a stale `http/auth.rs` reference from the
  inbound adapter module diagram, keeping documentation aligned with the
  current codebase structure.
