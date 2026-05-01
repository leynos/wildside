# Replace offset pagination on `GET /api/v1/users` with the keyset pagination crate

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: IMPLEMENTED; DRAFT PR OPEN

## Purpose / big picture

Roadmap task 4.2.1 directs us to retire the unpaginated `Vec<User>` shape on
`GET /api/v1/users` and replace it with a keyset-paginated envelope built on
the workspace `pagination` crate (`backend/crates/pagination`). After this
change a session-authenticated client can issue
`GET /api/v1/users?limit=N` and follow opaque `next` and `prev` cursor links
through the entire ordered user set without the server ever performing a
`COUNT(*)` or `OFFSET` query. The page is ordered by `(created_at ASC,
id ASC)` so insertions during traversal cannot duplicate or skip records, and
the underlying SQL is index-assisted by a new composite index. Success is
observable in three ways:

1. The handler returns the JSON envelope `{ "data": [...], "limit": N,
   "links": { "self": "...", "next": "...", "prev": "..." } }` with omitted
   keys when no further page exists.
2. Forward and backward cursor traversal returns every user exactly once
   (BDD scenario passes against an embedded PostgreSQL fixture seeded with
   more rows than fit on a single page).
3. `make check-fmt`, `make lint`, and `make test` all pass; the new BDD
   feature exercises the full traversal path.

The existing `DieselUsersQuery` only ever returns the caller's own row (it
delegates to `UserRepository::find_by_id`); after this work it must return a
true ordered slice of the users table.

## Constraints

These invariants come from `docs/wildside-backend-architecture.md`,
`docs/keyset-pagination-design.md`, and `AGENTS.md`. Violating any one of
them requires escalation, not a workaround.

- The handler `backend/src/inbound/http/users.rs::list_users` must remain a
  thin coordinator: parse query, call a domain port, map to response. It
  must not import Diesel, bb8, or `crate::outbound::*`. The architecture
  lint (`make lint-architecture`) must continue to pass.
- All persistence work stays in `backend/src/outbound/persistence/`. Diesel
  query construction must not leak into the domain layer.
- The cursor remains opaque (base64url-encoded JSON via the pagination
  crate's `Cursor::encode` / `Cursor::decode`); no new on-the-wire format.
- Default page size is `pagination::DEFAULT_LIMIT` (20); maximum is
  `pagination::MAX_LIMIT` (100). Clients must not be able to request
  larger pages.
- Ordering is `(created_at ASC, id ASC)` for every page, including the first
  page (no cursor) and reverse traversals.
- `User` domain invariants must be preserved: identity through `UserId`,
  validated `DisplayName`, `serde(deny_unknown_fields)`. If `created_at`
  must be exposed on `User`, do so additively without breaking the existing
  `UserDto` contract.
- Connection acquisition uses the existing `DbPool` (bb8 over
  `AsyncPgConnection`). No new pool, no blocking calls inside async tasks.
- Documentation, comments, and any new copy use en-GB Oxford spelling per
  `docs/documentation-style-guide.md`.

## Tolerances (exception triggers)

- Scope: stop and escalate if the diff exceeds roughly 800 net lines of
  source (excluding generated migrations and feature files) or touches more
  than 20 files.
- Interface: stop if changing `UserRepository` requires modifying any other
  caller besides `DieselUsersQuery`, the new paginated query path, and the
  startup-mode wiring in `backend/src/server/state_builders.rs`.
- Dependencies: no new crates. The pagination crate is added as a backend
  dependency; that single Cargo edit is in scope. Anything beyond that
  (e.g., adding `qs`, `urlencoding`, etc.) escalates.
- Iterations: if `make test` still fails after three good-faith attempts,
  pause and document the failure mode in `Surprises & Discoveries` before
  continuing.
- Time: if any single milestone (M0--M5 below) takes more than four hours of
  active work, stop and re-evaluate the approach.
- Ambiguity: if the User domain entity needs a structural change beyond
  adding `created_at` (for example, surfacing `updated_at` or relaxing
  `deny_unknown_fields`), stop and request direction.

## Risks

- Risk: surfacing `created_at` on the `User` domain entity changes the
  serialised JSON contract.
  Severity: medium. Likelihood: high.
  Mitigation: add the field with `#[serde(rename = "createdAt")]` and a
  matching deserialise alias; assert the new shape with a snapshot or
  explicit JSON round-trip test; cross-check the OpenAPI schema in
  `frontend-pwa/openapi.json` does not need a parallel hand edit.

- Risk: omitting the composite index leaves the new query doing a sort plus
  filter scan on every request, which would silently regress production
  latency.
  Severity: high. Likelihood: medium.
  Mitigation: ship the migration as the first commit; assert via `EXPLAIN`
  in a one-off integration test that the planner uses
  `idx_users_created_at_id` (or document that Postgres chose the primary
  key index due to small fixture size and is acceptable in test).

- Risk: forward/backward link generation has subtle off-by-one bugs around
  page boundaries (the design doc explicitly flags this in the "Determine
  Page Boundaries" section).
  Severity: high. Likelihood: high.
  Mitigation: derive `next`/`prev` from a single helper that always uses
  `limit + 1` fetch semantics; cover with BDD scenarios for first page,
  middle page, last page, single-item page, and exact-boundary page.

- Risk: existing handler tests in `backend/src/inbound/http/users/tests.rs`
  and `backend/tests/diesel_login_users_adapters.rs` assert the old
  `Vec<User>` shape and will break.
  Severity: low. Likelihood: certain.
  Mitigation: update assertions in the same commit that changes the
  response; do not introduce a transitional dual-shape response.

- Risk: the `FixtureUsersQuery` test double currently returns a single
  static "Ada Lovelace" user; the new trait method must remain trivially
  satisfiable for handler-only tests that do not need a real database.
  Severity: low. Likelihood: high.
  Mitigation: keep the fixture's behaviour minimal -- return the same row
  wrapped in a one-page envelope with no cursors -- so existing handler
  unit tests need only response-shape adjustments.

- Risk: BDD scenarios using `pg-embedded-setup-unpriv` are slow to start
  and can flake on the shared test cluster.
  Severity: low. Likelihood: medium.
  Mitigation: reuse the existing `TemporaryDatabase` and template helpers
  in `backend/tests/support/embedded_postgres.rs`; do not provision a
  fresh cluster per scenario.

## Progress

- [x] 2026-05-01: Implementation started on branch
  `4-2-1-replace-users-offset-pagination-with-new-crate`. The existing
  plan's worker-agent split will be executed locally because this session only
  permits sub-agent delegation when explicitly requested by the user.
- [x] M0: Branch created from `main`; pagination crate added to
  `backend/Cargo.toml` and `backend` builds cleanly with the import in a
  scratch module (no behaviour change). `cargo check -p backend`, `make
  check-fmt`, `make lint`, and a clean rerun of `make test` passed on
  2026-05-01.
- [x] M1: Migration `add_users_created_at_id_index` added under
  `backend/migrations/`; `make test` still passes after the migration runs
  via the embedded-postgres fixtures.
- [x] 2026-05-01: M1 migration files added with
  `idx_users_created_at_id` on `(created_at, id)` and a matching down
  migration; `make fmt`, `make markdownlint`, `make check-fmt`, `make lint`,
  and `make test` passed.
- [x] M2: Domain and port updates -- `User` exposes `created_at`,
  `UserCursorKey` defined, `UsersQuery` and `UserRepository` extended with
  paginated reads, `FixtureUsersQuery` updated.
- [x] 2026-05-01: M2 kept the first port change additive by defining
  default paginated trait methods that return a stable internal/query error
  until the Diesel adapter is implemented in M3. `FixtureUsersQuery` overrides
  the new query method immediately so handler-only tests have a deterministic
  fallback path.
- [x] 2026-05-01: M2 completed with `UserCursorKey`,
  `ListUsersPageRequest`, and `UsersPage`; `UserDto` accepts legacy payloads
  without `createdAt` but serialises the new field as `createdAt`. `make fmt`,
  `make markdownlint`, `make check-fmt`, `make lint`, and `make test` passed.
- [x] M3: Diesel adapter implements the keyset query (`limit + 1` fetch,
  composite filter, asc ordering); covered by unit tests with a stubbed
  `UserRepository` for error mapping and an integration test against
  embedded Postgres.
- [x] 2026-05-01: M3 implemented `DieselUserRepository::list_page` using
  `(created_at, id)` keyset predicates, one-row overflow fetches, and stable
  ascending return order for both forward and reverse pages. Reverse pages
  query descending for index-friendly "before cursor" access, then reverse
  rows before returning to the query port.
- [x] 2026-05-01: M3 implemented `DieselUsersQuery::list_users_page` overflow
  trimming and error mapping. Forward pages trim the trailing overflow row;
  reverse pages trim the leading overflow row because the repository has
  already restored ascending order. `cargo check -p backend`, focused
  `diesel_users_query` and `diesel_user_repository` tests, `make fmt`, `make
  check-fmt`, `make lint`, and `make test` passed. The full test gate ran
  1202 Rust tests successfully before the frontend and token workspace tests
  also passed.
- [x] M4: `list_users` handler rewritten to consume pagination query params,
  decode cursor, call the port, build links from request URL, and return
  `Paginated<UserSchema>`; OpenAPI annotations updated; existing handler
  tests adjusted to the new envelope.
- [x] 2026-05-01: M4 moved users pagination HTTP concerns into
  `backend/src/inbound/http/users_pagination.rs`. The handler now decodes
  users cursors, rejects malformed or oversized limits with structured
  `ErrorSchema` responses, calls `UsersQuery::list_users_page`, and returns
  the `Paginated<User>` envelope with `self`, `next`, and `prev` links.
- [x] 2026-05-01: M4 updated OpenAPI schema coverage for `createdAt`,
  `PaginatedUsersResponse`, and `PaginationLinksSchema`; updated handler,
  startup-mode, and adapter-guardrail tests to assert the new `data` envelope
  shape. `cargo check -p backend`, users handler tests, affected startup and
  guardrail tests, and OpenAPI/schema-focused tests passed before the full
  commit gates.
- [x] 2026-05-01: M4 full gates passed: `make fmt`, `make markdownlint`,
  `make check-fmt`, `make lint`, and `make test`. The final `make test` run
  completed 1206 Rust tests successfully with 4 skipped, then passed the root,
  frontend, and token workspace tests.
- [x] M5: BDD feature
  `backend/tests/features/users_list_pagination.feature` and step
  definitions cover happy and unhappy paths; full gate replay
  (`make check-fmt`, `make lint`, `make test`) is green; roadmap entry
  4.2.1 marked done; draft PR opened.
- [x] 2026-05-01: M5 added
  `backend/tests/features/users_list_pagination.feature` and
  `backend/tests/users_list_pagination_bdd.rs` with split flow support. The
  scenarios cover first page links, forward traversal to the final page,
  reverse traversal from the final page, oversized limit rejection, invalid
  cursor rejection, and unauthenticated access. The direct BDD test run passed
  14 tests.
- [x] 2026-05-01: Roadmap item 4.2.1 in `docs/backend-roadmap.md` marked
  complete after the endpoint, Diesel adapter, and BDD traversal coverage were
  in place.
- [x] 2026-05-01: M5 full gates passed after refactoring the BDD traversal
  helper to satisfy Clippy: `make fmt`, `make markdownlint`,
  `make check-fmt`, `make lint`, and `make test`. The final `make test` run
  completed 1220 Rust tests successfully with 4 skipped, then passed the root
  Vitest test, frontend workspace tests, TypeScript checks, and token contrast
  checks.
- [x] 2026-05-01: Draft PR
  [#349](https://github.com/leynos/wildside/pull/349) updated from the
  pre-implementation plan into the implementation review PR.

## Surprises & discoveries

- 2026-05-01: `leta` was available, but Rust indexing initially failed because
  `rust-analyzer` was missing from the active toolchain. Installing the rustup
  component and restarting the `leta` daemon restored Rust symbol lookup.
- 2026-05-01: The execplan says `GET /api/v1/users?limit=200` should return
  HTTP 400, while `backend/crates/pagination` and
  `docs/keyset-pagination-design.md` currently cap oversized limits to
  `MAX_LIMIT`. The implementation will follow the execplan's endpoint
  acceptance criteria and document any required pagination-crate behaviour
  change before it is made.
- 2026-05-01: The first full `make test` run after M0 failed in four
  embedded-PostgreSQL-backed tests while bootstrapping `/var/tmp/pg-embed-1000`
  (`pg_wal/... No such file or directory` and one `pg_ctl: another server might
  be running` report). No active PostgreSQL worker was left behind, and an
  immediate rerun passed all Rust and frontend tests without code changes, so
  this was treated as a transient fixture startup failure.
- 2026-05-01: Adding `created_at` to `User` exposed PostgreSQL's timestamp
  precision boundary: Diesel round-trips `timestamptz` values at microsecond
  precision, while `Utc::now()` supplies nanoseconds. `User::new` now
  normalises the domain timestamp to microsecond precision so persisted users,
  cursor keys, and test equality all use the same precision.
- 2026-05-01: `backend/tests/ports_behaviour.rs` had an independent
  PostgreSQL test adapter that still inserted only `id` and `display_name`.
  It now persists and reads `created_at`, using text casts because the direct
  `postgres` test client in this repository is not compiled with chrono
  `ToSql` / `FromSql` support.
- 2026-05-01: Diesel's reverse keyset query is clearest and cheapest when it
  asks PostgreSQL for rows before the cursor in descending order, applies the
  same `limit + 1` cap, and reverses the in-memory page. That leaves a reverse
  overflow row at the front of the returned ascending slice, so the query port
  must trim from the leading edge for `Direction::Prev`.
- 2026-05-01: Direct `web::Query<PageParams>` extraction would let Actix
  produce its default extractor body for malformed limits. The users endpoint
  needs the project `ErrorSchema` with `invalid_limit` details, so M4 parses a
  raw string limit in the inbound adapter and converts to `PageParams` after
  endpoint-specific validation.
- 2026-05-01: The M5 BDD fixture initially used a nested next-link traversal
  loop inside the step closure. Project Clippy runs with
  `clippy::excessive_nesting` as a hard error, so the traversal was extracted
  into a small async helper before the full lint gate was accepted.

## Decision log

- Decision: place the new `UserCursorKey` struct in
  `backend/src/domain/users_pagination.rs` (re-exported from
  `backend/src/domain/mod.rs`) rather than in the pagination crate or the
  outbound adapter.
  Rationale: the key is a domain concept (the natural ordering of users)
  and must be constructable from a `User` reference; placing it in the
  domain keeps the inbound handler and outbound adapter both depending
  inward, satisfying hexagonal layering. The pagination crate stays
  generic.
  Date/Author: 2026-04-28, drafting agent.

- Decision: extend the existing `UsersQuery` driving port with a new
  `list_users_page` method instead of replacing `list_users`.
  Rationale: `list_users` is also called by `diesel_login_users_adapters`
  startup-mode tests; keeping it allows the migration to land in a single
  PR without rewriting startup-mode coverage. The handler switches over;
  callers that genuinely want a single-user lookup keep working. We will
  remove `list_users` in a follow-up once no caller remains.
  Date/Author: 2026-04-28, drafting agent.

- Decision: do not add HMAC-signed cursors in this task.
  Rationale: the design doc explicitly defers signing to a future change;
  introducing it here would expand scope past tolerance and is not
  required by the roadmap.
  Date/Author: 2026-04-28, drafting agent.

- Decision: make M2's port additions additive by giving
  `UserRepository::list_page` and `UsersQuery::list_users_page` default
  implementations that return stable query/internal errors until the Diesel
  adapter is implemented.
  Rationale: this keeps the M2 commit focused on domain and port shape while
  avoiding a half-implemented persistence path. The fixture query overrides
  the method immediately, so handler-only tests still have a deterministic
  no-database path.
  Date/Author: 2026-05-01, implementation agent.

- Decision: normalise `User::created_at` to microsecond precision in the
  domain constructor.
  Rationale: users are persisted to PostgreSQL `timestamptz`, which stores
  microsecond precision. Normalising once at the domain boundary avoids
  adapter-specific timestamp drift and keeps cursor keys based on the same
  values that will be read back from storage.
  Date/Author: 2026-05-01, implementation agent.

- Decision: keep `UserRepository::list_page` rows in `(created_at ASC,
  id ASC)` order for both cursor directions.
  Rationale: stable repository ordering keeps response assembly simple and
  prevents inbound code from needing to know whether the page was fetched
  forwards or backwards. For reverse pages, the Diesel adapter performs the
  efficient descending SQL query internally, reverses the short page in
  memory, and lets `DieselUsersQuery` trim the leading overflow row.
  Date/Author: 2026-05-01, implementation agent.

- Decision: reject `GET /api/v1/users` limits above
  `pagination::MAX_LIMIT` in the users inbound adapter rather than changing
  the shared pagination crate.
  Rationale: the pagination crate deliberately normalises generic page params
  and existing documentation describes that behaviour. The users endpoint has
  a stricter acceptance criterion (`limit=200` returns HTTP 400 with
  structured details), so adapter-local validation satisfies the endpoint
  contract while preserving the crate's reusable default.
  Date/Author: 2026-05-01, implementation agent.

## Outcomes & retrospective

The users list endpoint now uses the workspace pagination crate end to end.
`GET /api/v1/users` returns a paginated envelope, uses opaque direction-aware
cursor tokens, and keeps `self`, `next`, and `prev` link generation in the
inbound HTTP adapter. The domain owns the user cursor key and the outbound
Diesel adapter owns all SQL, preserving the hexagonal boundary.

The storage path now has a composite `(created_at, id)` index and the Diesel
repository performs `limit + 1` keyset reads without `OFFSET` or `COUNT(*)`.
Forward and reverse pages are returned to callers in stable ascending order,
with overflow trimming handled in the query adapter.

The main implementation friction was reconciling the generic pagination
crate's limit-normalisation behaviour with the users endpoint's stricter
acceptance criterion. The endpoint now performs adapter-local raw limit
validation, which keeps the shared crate reusable while returning the required
structured `invalid_limit` response for oversized requests.

Validation finished cleanly on 2026-05-01: focused BDD coverage for the users
list passed, then `make fmt`, `make markdownlint`, `make check-fmt`,
`make lint`, and `make test` all passed.

## Context and orientation

The Wildside backend is a hexagonal modular monolith. The handler under
change is at `backend/src/inbound/http/users.rs:243-251`:

```rust
#[get("/users")]
pub async fn list_users(
    state: web::Data<HttpState>,
    session: SessionContext,
) -> ApiResult<web::Json<Vec<User>>> {
    let user_id = session.require_user_id()?;
    let data = state.users.list_users(&user_id).await?;
    Ok(web::Json(data))
}
```

`HttpState::users` is `Arc<dyn UsersQuery>`
(`backend/src/inbound/http/state.rs`). The driving port lives at
`backend/src/domain/ports/users_query.rs`:

```rust
#[async_trait]
pub trait UsersQuery: Send + Sync {
    async fn list_users(&self, authenticated_user: &UserId) -> Result<Vec<User>, Error>;
}
```

Two implementations exist:

- `FixtureUsersQuery` (same file) returns one static "Ada Lovelace" user.
- `DieselUsersQuery` (`backend/src/outbound/persistence/diesel_users_query.rs`)
  delegates to `UserRepository::find_by_id` -- it does not actually list
  rows today. It must learn how to do a real paginated read.

The driven port `UserRepository` lives at
`backend/src/domain/ports/user_repository.rs` and has only `upsert` and
`find_by_id`. The Diesel adapter
`backend/src/outbound/persistence/diesel_user_repository.rs` runs through
`DbPool` (`backend/src/outbound/persistence/pool.rs`), a bb8 pool over
`diesel_async::AsyncPgConnection`.

The schema is `backend/src/outbound/persistence/schema.rs`:

```rust
diesel::table! {
    users (id) {
        id -> Uuid,
        display_name -> Varchar,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}
```

The migration `backend/migrations/2025-12-10-000000_create_users/up.sql`
creates the table and an index on `display_name` only. There is **no**
composite index on `(created_at, id)` today.

The pagination crate (`backend/crates/pagination`) provides:

- `Direction` (`Next` | `Prev`), `Cursor<K>::encode/decode`,
  `PageParams { cursor, limit }` with `DEFAULT_LIMIT = 20` and
  `MAX_LIMIT = 100`,
- `Paginated<T> { data, limit, links }` and
  `PaginationLinks::from_request(url, params, next, prev)` for link
  generation.

It is **not** yet declared in `backend/Cargo.toml`. Add it as
`pagination = { path = "crates/pagination" }`.

User-visible response shape today is a raw JSON array. After the change it
becomes:

```json
{
  "data": [{ "id": "...", "displayName": "..." }],
  "limit": 20,
  "links": {
    "self": "/api/v1/users?limit=20",
    "next": "/api/v1/users?cursor=eyJk...&limit=20"
  }
}
```

`prev` is omitted on the first page; `next` is omitted on the last page.
Field names follow camelCase via the existing serde defaults on
`PaginationLinks` (`self_` serialises as `"self"`).

### Signposts (read these before starting)

- `docs/keyset-pagination-design.md` -- canonical design for the crate and
  the integration pattern (especially section "Integrating Pagination in
  Handlers").
- `docs/wildside-backend-architecture.md` -- hexagonal layering rules and
  inbound/outbound module map; consult before placing new types.
- `docs/backend-roadmap.md` section 4 -- task scope and downstream items
  (4.2.2 onwards) that must remain implementable after this change.
- `docs/pg-embed-setup-unpriv-users-guide.md` -- how to spin up a temporary
  PostgreSQL for integration tests.
- `docs/rstest-bdd-users-guide.md` -- BDD step authoring patterns used in
  this repo.
- `docs/rust-testing-with-rstest-fixtures.md` -- shared fixture style.
- `docs/rust-doctest-dry-guide.md` -- doctest patterns; the handler's
  existing example must continue to compile.
- `docs/complexity-antipatterns-and-refactoring-strategies.md` -- guidance
  on extracting helpers when the new handler logic grows beyond a screen.
- `backend/crates/pagination/src/lib.rs` -- crate-level docs and public
  API.

### Skills (load when relevant)

- `hexagonal-architecture` -- use throughout; verify each new type and
  function lands in the correct ring (domain / port / adapter / inbound).
- `rust-router` -- entry point for the focused Rust skills below.
- `rust-types-and-apis` -- when extending `UsersQuery`, `UserRepository`,
  and the `User` struct; helps shape trait bounds and conversions.
- `rust-async-and-concurrency` -- when adding the new async repository
  method and ensuring no blocking work runs inside a Tokio task.
- `rust-errors` -- when mapping cursor decode failures and pagination
  parameter errors onto domain `Error` variants and HTTP 400 responses.
- `nextest` -- for running `make test` and triaging individual test
  failures during M3--M5.
- `en-gb-oxendict` -- for any documentation, comments, and feature-file
  copy.
- `commit-message` -- to write the per-milestone commits.
- `pr-creation` -- to open the final PR.

## Plan of work

The work proceeds in five milestones. Each milestone ends with the same
gate: `make check-fmt`, `make lint`, and `make test` must succeed before
the next milestone begins, and a single focused commit captures the
change.

### Stage A: prepare (M0 -- pagination crate available to backend)

1. Branch from current `main`. Suggested name:
   `backend-4-2-1-users-keyset-pagination`. Confirm with the user before
   pushing if scope warrants.
2. Add the pagination crate to `backend/Cargo.toml`:
   `pagination = { path = "crates/pagination" }`. Workspace already
   contains the crate, so no workspace edit is needed.
3. Run `cargo check -p backend` to confirm the dep resolves.
4. Commit: `Add pagination crate dependency to backend`.

Validation: `make check-fmt && make lint && make test` pass; the new
dependency is visible in `cargo tree -p backend | grep pagination`.

### Stage B: schema (M1 -- composite index)

1. Generate a new Diesel migration directory under `backend/migrations/`,
   e.g., `2026-04-28-000000_add_users_created_at_id_index/`.
2. `up.sql`: `CREATE INDEX IF NOT EXISTS idx_users_created_at_id ON users
   (created_at, id);`. `down.sql`: `DROP INDEX IF EXISTS
   idx_users_created_at_id;`.
3. Confirm `EmbeddedMigrations` picks up the new directory automatically
   (it uses `embed_migrations!` over the directory tree -- no Rust change
   needed beyond running tests so the embedded fixtures re-run
   migrations).
4. Commit: `Add composite (created_at, id) index for users keyset
   pagination`.

Validation: every existing test passes; `backend/tests/diesel_user_repository.rs`
runs the new migration without error.

### Stage C: domain and ports (M2)

This stage is dispatched to two parallel worker agents under the lead
agent's coordination, since the changes are mostly additive and touch
disjoint files:

- Worker A (domain types): owns `backend/src/domain/user.rs` and a new
  module `backend/src/domain/users_pagination.rs`.
- Worker B (ports): owns `backend/src/domain/ports/users_query.rs`,
  `backend/src/domain/ports/user_repository.rs`, and the
  `FixtureUsersQuery` impl.

Worker A tasks:

1. Add `created_at: chrono::DateTime<chrono::Utc>` to the `User` struct;
   thread it through the constructor (`User::new`) and the `UserDto`
   serialisation form. Update existing factories
   (`docs/backend-sample-data-design.md` describes the example-data crate;
   confirm any factory-style helper continues to compile).
2. Add `User::created_at(&self) -> chrono::DateTime<chrono::Utc>`
   accessor.
3. Create `backend/src/domain/users_pagination.rs` defining:

   ```rust
   pub struct UserCursorKey {
       pub created_at: chrono::DateTime<chrono::Utc>,
       pub id: uuid::Uuid,
   }
   impl From<&User> for UserCursorKey { /* ... */ }
   ```

   Derive `Serialize`, `Deserialize`, `Debug`, `Clone`. Add a doctest
   showing round-trip via `pagination::Cursor::encode/decode`.
4. Re-export `UserCursorKey` from `backend/src/domain/mod.rs`.

Worker B tasks:

1. Extend `UserRepository` with:

   ```rust
   async fn list_page(
       &self,
       request: ListUsersPageRequest,
   ) -> Result<Vec<User>, UserPersistenceError>;
   ```

   where `ListUsersPageRequest { cursor: Option<Cursor<UserCursorKey>>,
   limit: usize }` lives next to the trait. The method returns up to
   `limit + 1` rows so callers can detect overflow.
2. Extend `UsersQuery` with:

   ```rust
   async fn list_users_page(
       &self,
       authenticated_user: &UserId,
       request: ListUsersPageRequest,
   ) -> Result<UsersPage, Error>;
   ```

   where `UsersPage { rows: Vec<User>, has_more: bool }` is a small
   value type defined in the same file. The intent is to keep
   "did we fetch one extra?" logic encapsulated, so the handler does
   not need to peek.
3. Update `FixtureUsersQuery` so `list_users_page` returns the static row
   on the first page (no cursor) and an empty page otherwise.
4. Keep the existing `list_users` method intact (decision-log entry
   above).

Lead agent reviews both worker patches together, resolves any naming
conflicts, and commits.

Validation: `make check-fmt && make lint && make test` pass. Existing
handler tests still compile because the handler has not yet been
rewritten.

### Stage D: persistence adapter (M3)

In `backend/src/outbound/persistence/diesel_users_query.rs` and
`diesel_user_repository.rs`:

1. Implement `UserRepository::list_page` in `DieselUserRepository`. Use
   `users::table.into_boxed()`, apply the `(created_at, id)` lexicographic
   filter for `Direction::Next` or `Direction::Prev`, order by
   `created_at.asc()` then `id.asc()`, and limit to `limit + 1`. Map
   `bb8` errors via the existing `map_pool_error`/`map_diesel_error`
   helpers.
2. Implement `UsersQuery::list_users_page` in `DieselUsersQuery`. Decode
   the boundary semantics: take the (up to) `limit + 1` rows from the
   repository, set `has_more = rows.len() > limit`, truncate to `limit`,
   and return `UsersPage`.
3. Unit tests in `diesel_users_query.rs` extend the existing
   `StubUserRepository` to assert that pool/connection errors map to
   `Error::ServiceUnavailable` and query errors map to
   `Error::InternalError`, mirroring the existing pattern.
4. Add an integration test
   `backend/tests/diesel_users_query_pagination.rs` that seeds at least
   `MAX_LIMIT + 5` users with controlled `created_at` values, walks
   forward to the end, and walks back to the start using the same cursor
   strings the handler would emit. Use the existing
   `TemporaryDatabase`/`with_context_async` machinery in
   `backend/tests/diesel_user_repository.rs` as the model.

Validation: the new integration test fails without the keyset filter and
passes with it; the unit tests pass; `make test` is green.

Commit: `Implement keyset-paginated users listing in Diesel adapter`.

### Stage E: handler, OpenAPI, and BDD (M4 + M5)

Inbound handler changes (`backend/src/inbound/http/users.rs`):

1. Replace the `list_users` body with:

   ```rust
   pub async fn list_users(
       state: web::Data<HttpState>,
       session: SessionContext,
       request: HttpRequest,
       params: web::Query<PageParams>,
   ) -> ApiResult<web::Json<Paginated<UserSchema>>> { /* ... */ }
   ```

   Decode the cursor via `Cursor::<UserCursorKey>::decode` and map errors
   to `Error::invalid_request` with HTTP 400 and structured details
   (`field: "cursor", code: "invalid_cursor"`). Map `PageParamsError` the
   same way (`field: "limit"`, `code: "invalid_limit"`).
2. Build links via `PaginationLinks::from_request`, passing `request.uri()`
   converted to `url::Url`. Extract a small helper
   `current_request_url(req: &HttpRequest) -> url::Url` if it grows beyond
   four lines.
3. Update the utoipa `#[utoipa::path]` annotations: declare `cursor` and
   `limit` query parameters, and replace the `body = UsersListResponse`
   response with `body = PaginatedUsersResponse`. Define
   `PaginatedUsersResponse` as a thin schema token that mirrors
   `Paginated<UserSchema>` (use the same `PartialSchema`/`ToSchema`
   pattern the existing `UsersListResponse` uses, so the generated
   OpenAPI matches the design doc's `PaginatedUsers` example).
4. Delete `UsersListResponse` and the `USERS_LIST_MAX` constant; the
   pagination crate's `MAX_LIMIT` is the single source of truth.
5. Update `backend/src/inbound/http/users/tests.rs` to assert the new
   envelope shape (data length, presence/absence of `next`/`prev`).

Behavioural tests (M5):

1. Add `backend/tests/features/users_list_pagination.feature`. Scenarios:
   - First page returns `limit` rows, includes `next`, omits `prev`.
   - Following `next` reaches the final page, which includes `prev` and
     omits `next`.
   - Following `prev` from the final page returns the prior page intact.
   - Requesting `limit=200` returns HTTP 400 with the
     `invalid_limit` detail code.
   - Requesting an unparseable `cursor` returns HTTP 400 with the
     `invalid_cursor` detail code.
   - Unauthenticated request returns HTTP 401 (regression for existing
     session behaviour).
2. Add `backend/tests/users_list_pagination_bdd.rs` with step definitions.
   Reuse `support::embedded_postgres` to seed users with deterministic
   `created_at` values (e.g., one minute apart starting at a fixed UTC
   instant) so cursor traversal is reproducible.
3. Update `backend/tests/diesel_login_users_adapters.rs` if it asserts
   the old response shape.

Documentation:

1. Append a short note in `docs/wildside-backend-architecture.md` (in the
   pagination or read-model section) recording: "User listing uses keyset
   pagination on `(created_at, id)`; see
   `docs/keyset-pagination-design.md`. The driving port `UsersQuery`
   exposes `list_users_page` returning `UsersPage`; the legacy
   `list_users` is retained until callers migrate."
2. Mark roadmap entry 4.2.1 as `[x]` with the date.

Final commit + PR:

1. Run the full gate (`make check-fmt`, `make lint`, `make test`),
   piping each output through `tee /tmp/<action>-backend-4-2-1.out` per
   `AGENTS.md`.
2. Commit the handler/OpenAPI/BDD work as one atomic change:
   `Adopt keyset pagination on GET /api/v1/users`.
3. Open a PR via the `pr-creation` skill referencing roadmap §4.2.1 and
   this ExecPlan.

## Concrete steps

Run from the worktree root unless noted.

```bash
git branch --show-current
# Confirm we are NOT on main; if we are, branch:
git switch -c backend-4-2-1-users-keyset-pagination
```

Add the dep:

```bash
# Edit backend/Cargo.toml manually -- add:
#   pagination = { path = "crates/pagination" }
cargo check -p backend
```

Generate the migration directory:

```bash
mkdir -p backend/migrations/2026-04-28-000000_add_users_created_at_id_index
# Write up.sql and down.sql as described in Stage B.
```

Run gates after each milestone:

```bash
make check-fmt 2>&1 | tee /tmp/check-fmt-backend-4-2-1-users-keyset-pagination.out
make lint     2>&1 | tee /tmp/lint-backend-4-2-1-users-keyset-pagination.out
make test     2>&1 | tee /tmp/test-backend-4-2-1-users-keyset-pagination.out
```

Expected: every command exits 0. The `test` invocation is the slow one;
do not run it in parallel with another test job per `AGENTS.md`.

When the BDD scenarios are in place, exercise just the new feature
quickly while iterating:

```bash
cargo nextest run -p backend --test users_list_pagination_bdd \
    --no-fail-fast 2>&1 | tee /tmp/nextest-users-pagination.out
```

## Validation and acceptance

Quality criteria (what "done" means):

- `make check-fmt`, `make lint`, and `make test` all pass on the final
  commit, evidenced by the captured `/tmp/*.out` logs.
- A user with a session cookie can call `GET /api/v1/users` and receive the
  envelope described in `Purpose / big picture`. Following `next` and
  `prev` links recovers the same user set as a single un-paginated SQL
  query (asserted in the BDD scenarios).
- `GET /api/v1/users?limit=200` returns HTTP 400 with body
  `{"error":{...,"details":{"field":"limit","code":"invalid_limit", ...}}}`.
- `GET /api/v1/users?cursor=not-base64` returns HTTP 400 with
  `code: "invalid_cursor"`.
- Unauthenticated requests still receive HTTP 401 (regression).
- `EXPLAIN (ANALYZE, BUFFERS)` on the keyset query (run manually once,
  recorded in `Surprises & Discoveries`) shows an index scan on
  `idx_users_created_at_id`, not a full table scan, when the table has
  more than a few thousand rows.

Quality method (how we check):

- Integration tests in `backend/tests/diesel_users_query_pagination.rs`
  execute the SQL path against an embedded PostgreSQL.
- BDD feature `backend/tests/features/users_list_pagination.feature`
  exercises the HTTP path end-to-end via the Actix test server.
- The architecture lint (`make lint-architecture`) confirms the inbound
  handler does not import outbound modules.

## Idempotence and recovery

- The new migration is gated by `IF NOT EXISTS` / `IF EXISTS`, so re-running
  the embedded test cluster after a partial run is safe.
- Each milestone ends with a clean commit. If a milestone gate fails,
  revert the milestone with `git restore --source=HEAD~1` rather than
  amending; do not push partial milestones.
- The cursor format is fully recoverable: clients re-issuing a stale
  cursor always either succeed or receive a deterministic HTTP 400; no
  server-side state needs reconciliation.
- `pg-embedded-setup-unpriv` test clusters are auto-cleaned via the
  existing `atexit_cleanup` machinery in `backend/tests/support`.

## Artifacts and notes

Expected JSON envelope (first page, default limit):

```json
{
  "data": [
    { "id": "11111111-1111-1111-1111-111111111111", "displayName": "Ada" }
  ],
  "limit": 20,
  "links": {
    "self": "/api/v1/users?limit=20",
    "next": "/api/v1/users?cursor=eyJkaXIiOiJOZXh0Iiwia2V5Ijp7ImNyZWF0ZWRfYXQiOiIyMDI2LTA0LTI4VDAwOjAwOjAwWiIsImlkIjoiMTExMTExMTEtMTExMS0xMTExLTExMTEtMTExMTExMTExMTExIn19&limit=20"
  }
}
```

(`prev` is omitted when null per the crate's `skip_serializing_if`
configuration.)

Expected error envelope (invalid cursor):

```json
{
  "error": {
    "code": "INVALID_REQUEST",
    "message": "invalid pagination cursor",
    "details": { "field": "cursor", "code": "invalid_cursor" }
  }
}
```

## Interfaces and dependencies

In `backend/src/domain/users_pagination.rs`, define:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserCursorKey {
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub id: uuid::Uuid,
}

impl From<&crate::domain::User> for UserCursorKey {
    fn from(user: &crate::domain::User) -> Self {
        Self { created_at: user.created_at(), id: user.id().as_uuid() }
    }
}
```

In `backend/src/domain/ports/users_query.rs`, extend the trait to:

```rust
use pagination::Cursor;

pub struct ListUsersPageRequest {
    pub cursor: Option<Cursor<UserCursorKey>>,
    pub limit: usize,
}

pub struct UsersPage {
    pub rows: Vec<User>,
    pub has_more: bool,
}

#[async_trait]
pub trait UsersQuery: Send + Sync {
    async fn list_users(&self, authenticated_user: &UserId) -> Result<Vec<User>, Error>;
    async fn list_users_page(
        &self,
        authenticated_user: &UserId,
        request: ListUsersPageRequest,
    ) -> Result<UsersPage, Error>;
}
```

In `backend/src/domain/ports/user_repository.rs`, extend the trait with:

```rust
async fn list_page(
    &self,
    request: ListUsersPageRequest,
) -> Result<Vec<User>, UserPersistenceError>;
```

In `backend/src/inbound/http/users.rs`, the rewritten handler signature:

```rust
pub async fn list_users(
    state: web::Data<HttpState>,
    session: SessionContext,
    request: HttpRequest,
    params: web::Query<PageParams>,
) -> ApiResult<web::Json<Paginated<UserSchema>>>;
```

External crate dependencies introduced: only the workspace-local
`pagination` crate. No new third-party crates.

## Revision note

(none yet)
