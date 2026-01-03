# Phase 1 session-guarded user profile and interests

This execution plan (ExecPlan) is a living document. The sections `Progress`,
`Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective` must
be kept up to date as work proceeds.

There is no `PLANS.md` in this repository, so this ExecPlan is the sole
execution reference.

## Purpose / Big Picture

Require a valid session for `/api/v1/users/me` and
`/api/v1/users/me/interests`, and return `401 Unauthorized` responses that
include trace identifiers when a session is missing or invalid. The handler
logic must remain adapter-thin and must call domain ports for any stateful
behaviour.

Observable success:

- Calling `GET /api/v1/users/me` or `PUT /api/v1/users/me/interests` without a
  session returns HTTP 401, includes a `trace-id` response header, and the JSON
  error body contains `traceId`.
- Calling the same endpoints with a valid session returns a success response
  and uses injected domain ports (no direct persistence logic in handlers).
- New unit tests (rstest) cover domain validation and handler mapping.
- New behavioural tests (rstest-bdd v0.2.0) cover authenticated and
  unauthenticated flows.
- `docs/wildside-backend-architecture.md` records the design decisions made.
- `docs/backend-roadmap.md` marks the “Session lifecycle hardening” item as
  done.
- `make check-fmt`, `make lint`, and `make test` succeed.

## Progress

- [x] (2025-12-19) Review existing HTTP session middleware and Trace
  propagation to confirm how `401` responses get trace IDs.
- [x] (2025-12-19) Define domain types and ports for current user profile and
  interest selection.
- [x] (2025-12-19) Implement `/api/v1/users/me` and
  `/api/v1/users/me/interests` handlers that use session + ports.
- [x] (2025-12-19) Add OpenAPI schema wrappers and update `ApiDoc` paths.
- [x] (2025-12-19) Add unit tests with `rstest` and behaviour tests with
  `rstest-bdd` v0.2.0 (including trace-id assertions).
- [x] (2025-12-19) Record architecture decision(s) and mark roadmap item done.
- [x] (2025-12-19) Run `make check-fmt`, `make lint`, `make test` (with
  `set -o pipefail` and `tee`) and fix failures. (Initial `make test` needed a
  longer timeout; reran with 900 seconds.)

## Surprises & Discoveries

- Observation: `rstest-bdd` matches fixtures by parameter name; using `_world`
  in a step function caused a missing fixture error. Evidence:
  `Missing fixture '_world'` failure in `user_session_bdd` before renaming the
  parameter to `world`.
- Observation: `make lint` used the PATH-installed `redocly` binary, which
  failed due to a React version mismatch; forcing `bun x` resolved the lint.
  Evidence: `Incompatible React versions` from `redocly` when the global binary
  was used.

## Decision Log

- Decision: Introduce explicit driving ports for the authenticated user
  profile and interest selection rather than letting handlers touch persistence
  or shared state. Rationale: Keeps inbound adapters side effect free and
  preserves the hexagonal boundary by routing stateful behaviour through domain
  ports. Date/Author: 2025-12-19 / Codex CLI.

- Decision: Model interest theme identifiers as a domain newtype around a
  universally unique identifier (UUID) and validate them before reaching port
  implementations. Rationale: Validation belongs in the domain to ensure
  consistent behaviour across adapters and future persistence layers.
  Date/Author: 2025-12-19 / Codex CLI.

- Decision: Use the `world` parameter name in rstest-bdd step functions to
  align with fixture naming rules. Rationale: rstest-bdd resolves fixtures by
  parameter name, so `_world` would look for a missing fixture. Date/Author:
  2025-12-19 / Codex CLI.

## Outcomes & Retrospective

- Session enforcement is now wired for `/api/v1/users/me` and
  `/api/v1/users/me/interests`, with domain ports supplying stateful behaviour
  and trace IDs propagated on unauthorised responses.
- New domain types, ports, fixtures, and OpenAPI schema wrappers document the
  interest selection workflow.
- Behavioural coverage includes authenticated/unauthenticated scenarios, and
  unit tests cover validation and schema expectations.
- `make check-fmt`, `make lint`, and `make test` now pass.

## Context and Orientation

Key paths (repository-relative):

- `backend/src/inbound/http/users.rs`: current login + list users handlers;
  likely location for `/users/me` and `/users/me/interests` handlers.
- `backend/src/inbound/http/session.rs`: session wrapper (`SessionContext`) and
  `require_user_id()` for enforcing authentication.
- `backend/src/middleware/trace.rs`: Trace middleware that sets `trace-id` and
  scopes `TraceId` for error responses.
- `backend/src/inbound/http/state.rs`: adapter state container for injected
  domain ports.
- `backend/src/domain/ports/*`: driving ports for HTTP handlers; add new ones
  here with fixture implementations.
- `backend/src/doc.rs` and `backend/src/inbound/http/schemas.rs`: OpenAPI paths
  and schema wrappers.
- `backend/tests/adapter_guardrails/*`: patterns for integration-style tests
  with mocked ports.
- `backend/tests/features/*`: Gherkin feature files for `rstest-bdd`.
- `docs/wildside-backend-architecture.md`: design decisions; update after
  choosing the new ports and data shapes.
- `docs/backend-roadmap.md`: mark the Phase 1 item done once all checks pass.

Terminology (plain language):

- *Port*: a trait defining a boundary between adapters and domain logic.
- *Adapter*: HTTP handler code that parses requests and delegates to ports.
- *Trace identifier*: a request-scoped universally unique identifier (UUID)
  exposed in the `trace-id` response header and the error payload `traceId`
  field.

## Plan of Work

1. Confirm how `Trace` middleware and `SessionContext` currently behave by
   locating the existing handler wiring in `backend/src/server/mod.rs` and test
   harnesses. Note where Trace is (and is not) applied so tests can assert
   trace IDs consistently.

2. Define domain primitives and ports for these endpoints:

   - Add an `InterestThemeId` newtype (universally unique identifier (UUID)
     wrapper) in the domain, with validation similar to `UserId`. Add a small
     aggregate such as `UserInterests` (user id + interest theme ids) if that
     improves clarity.
   - Add a driving port for loading the current user profile, e.g.
     `UserProfileQuery::fetch_profile(&UserId) -> Result<User, Error>`.
   - Add a driving port for updating interest selections, e.g.
     `UserInterestsCommand::set_interests(&UserId, Vec<InterestThemeId>) ->
     Result<UserInterests, Error>`.
   - Provide fixture implementations in `backend/src/domain/ports` that return
     deterministic data so handlers can be tested without persistence.

3. Extend `HttpState` to include the new ports and update construction sites
   (`backend/src/server/mod.rs` and any test harness) so the handlers receive
   those injected services.

4. Implement the handlers in `backend/src/inbound/http/users.rs`:

   - `GET /api/v1/users/me`:
     - Require `SessionContext` and call `require_user_id()`.
     - Call the profile port and return `UserSchema` JSON on success.
   - `PUT /api/v1/users/me/interests`:
     - Require `SessionContext` and validate the request body into domain
       types (interest theme IDs).
     - Call the interest-update port and return the updated interests payload
       (or `204 No Content` if the API design prefers a write-only response).

   Ensure both handlers use the standard `Error` type so `401` responses carry
   the trace ID captured by the `Trace` middleware.

5. Update OpenAPI definitions:

   - Add schema wrappers for any new domain types (e.g.
     `InterestThemeId`, `UserInterests`) in
     `backend/src/inbound/http/schemas.rs`.
   - Register new paths and schemas in `backend/src/doc.rs` and update the
     BDD OpenAPI feature/tests to assert the new endpoints reference the right
     schema wrappers.

6. Tests:

   - Unit tests (`rstest`):
     - Validate `InterestThemeId` parsing and invalid UUID handling.
     - Validate request-body conversion failures map to
       `Error::invalid_request` with details.
     - Validate fixture ports return deterministic outputs.
   - Behaviour tests (`rstest-bdd` v0.2.0):
     - Add a `.feature` file under `backend/tests/features/` covering:
       - Unauthenticated `GET /api/v1/users/me` → 401 + trace id.
       - Unauthenticated `PUT /api/v1/users/me/interests` → 401 + trace id.
       - Authenticated flows for both endpoints.
     - Implement step definitions in a new test module, reusing the existing
       adapter guardrails harness or adding a minimal harness that wraps the
       app with both Session and Trace middleware so trace IDs are present.

   For any tests that need Postgres, use
   `pg_embedded_setup_unpriv::TestCluster` or its `rstest` fixture. If no new
   database interactions are added, ensure existing embedded Postgres tests
   still pass and document the local `PG_WORKER_PATH` override when running
   `make test` without elevated permissions.

7. Documentation updates:

   - Add a design decision entry in
     `docs/wildside-backend-architecture.md` describing the new ports and
     interest theme identifier model.
   - Mark the Phase 1 roadmap item as done in `docs/backend-roadmap.md` once
     all quality gates pass.

## Concrete Steps

Run all commands from the repository root. Use a default timeout of 300 seconds
per command unless a step explicitly requires more time.

1. Apply code and documentation changes.

2. Format (write changes) after any Rust or Markdown edits:

    timeout 300 make fmt

3. Verify formatting (no writes):

    timeout 300 make check-fmt

4. Lint (Clippy, rustdoc, architecture lint, Biome, infra linters):

    set -o pipefail
    timeout 300 make lint 2>&1 | tee /tmp/wildside-make-lint.log

5. Test (Rust + JS tests + scripts tests):

    set -o pipefail
    timeout 300 make test 2>&1 | tee /tmp/wildside-make-test.log

If Postgres tests need unprivileged setup locally, run:

    PG_WORKER_PATH=/tmp/pg_worker timeout 300 make test 2>&1 | tee \\
      /tmp/wildside-make-test.log

## Validation and Acceptance

Acceptance criteria:

1. Behavioural checks:
   - `GET /api/v1/users/me` without a session returns 401, includes `trace-id`
     header, and the JSON error payload includes `traceId`.
   - `PUT /api/v1/users/me/interests` without a session returns 401 with the
     same trace-id guarantees.
   - Authenticated calls return success and the port implementations record
     calls with the authenticated `UserId`.

2. Test coverage:
   - New rstest unit tests cover domain validation and request mapping.
   - New rstest-bdd v0.2.0 scenarios cover both authenticated and
     unauthenticated flows.

3. Quality gates:
   - `make check-fmt` passes.
   - `make lint` passes.
   - `make test` passes.

4. Documentation and roadmap:
   - `docs/wildside-backend-architecture.md` includes the new decision entry.
   - The Phase 1 roadmap line for session lifecycle hardening is marked done.

## Idempotence and Recovery

- All steps are safe to re-run.
- If Postgres bootstrapping fails locally, confirm `tzdata` is installed or
  set `TZDIR` explicitly, then retry with the `PG_WORKER_PATH` override.
- If a command times out, re-run with a higher timeout and keep the log file
  for inspection.

## Artifacts and Notes

- Existing patterns for adapter tests live in
  `backend/tests/adapter_guardrails/*` and can be reused for new endpoints.
- The `Trace` middleware must be applied in the test harness if trace-id
  assertions are added (see `backend/src/middleware/trace.rs`).

## Interfaces and Dependencies

Add or update the following interfaces (final state target):

- In `backend/src/domain/interest_theme.rs` (new module), define:

    pub struct InterestThemeId(Uuid, String);

  plus validation helpers similar to `UserId`.

- In `backend/src/domain/ports/user_profile_query.rs` (new file), define:

    #[async_trait::async_trait]
    pub trait UserProfileQuery: Send + Sync {
        async fn fetch_profile(&self, user_id: &UserId) -> Result<User, Error>;
    }

  Include a `FixtureUserProfileQuery` implementation returning a deterministic
  `User`.

- In `backend/src/domain/ports/user_interests_command.rs` (new file), define:

      #[async_trait::async_trait]
      pub trait UserInterestsCommand: Send + Sync {
          async fn set_interests(
              &self,
              user_id: &UserId,
              interest_theme_ids: Vec&lt;InterestThemeId&gt;,
          ) -> Result&lt;UserInterests, Error&gt;;
      }

  Include a fixture implementation returning a deterministic `UserInterests`.

- In `backend/src/inbound/http/users.rs`, add handlers:

      #[get("/users/me")]
      async fn current_user(
          state: web::Data&lt;HttpState&gt;,
          session: SessionContext,
      ) -> ApiResult&lt;web::Json&lt;User&gt;&gt; { … }

      #[put("/users/me/interests")]
      async fn update_interests(
          state: web::Data&lt;HttpState&gt;,
          session: SessionContext,
          payload: web::Json&lt;InterestsRequest&gt;,
      ) -> ApiResult&lt;web::Json&lt;UserInterests&gt;&gt; { … }

  with request data transfer objects (DTOs) that convert into domain types via
  `TryFrom` so validation errors map to `Error::invalid_request`.

- In `backend/src/inbound/http/state.rs`, extend `HttpState` with the new
  ports so handlers can be wired via dependency injection.

Update dependencies if required to move to `rstest-bdd = "0.2.0"` (and
`rstest-bdd-macros = "0.2.0"`), then update any tests that rely on the older
API surface.

## Revision note (required when editing an ExecPlan)

- Initial version authored on 2025-12-19 to plan Phase 1 session lifecycle
  hardening for `/api/v1/users/me` and `/api/v1/users/me/interests`.
- Updated on 2025-12-19 to mark work complete, capture new decisions and
  surprises, and record the final validation results (including the extended
  test timeout).
- Updated on 2025-12-20 to expand acronyms (ExecPlan, UUID, DTOs) per review
  guidance.
- Updated on 2025-12-20 to escape generic angle brackets for markdownlint and
  align wording with review feedback.
