# Phase 0: Update Modules to Use Domain Ports

This execution plan completes the roadmap item: "Update all modules to depend on
the domain ports rather than reaching into `backend/src/models` or
framework-specific types, ensuring the dependency flow points inward."

## Summary

Two categories of hexagonal architecture violations exist:

1. **TraceId crosses middleware→domain boundary**: Domain code imports `TraceId`
   from `middleware/trace.rs`
2. **OpenAPI (Open API Specification) / Utoipa framework types in domain**: Domain
   types derive `ToSchema` from the `utoipa` crate

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| OpenAPI schemas | Pure domain with external schemas | Domain types remain framework-agnostic; schemas defined in inbound layer |
| tokio in domain | Acceptable | Async runtime infrastructure, not framework coupling |
| Lint enforcement | Forbid `utoipa` in domain | Enforces hexagonal boundary going forward |

---

## Step 1: Relocate TraceId to Domain

Move the pure `TraceId` type from `middleware/trace.rs` to domain. The type uses
only `uuid` and `tokio::task_local!`—no Actix dependencies.

### 1.1 Create domain/trace_id.rs

**File**: `backend/src/domain/trace_id.rs`

Move from `middleware/trace.rs`:

- `task_local! { static TRACE_ID: TraceId; }` declaration
- `TraceId` struct definition (lines 38-39)
- `impl TraceId` with `generate()`, `from_uuid()`, `current()`, `as_uuid()`,
  `scope()`
- `impl Display`, `impl FromStr` for `TraceId`
- Unit tests for `TraceId` (non-Actix tests from lines 186-214, 281-300)

### 1.2 Update domain/mod.rs

Add:

```rust
pub mod trace_id;
pub use self::trace_id::TraceId;
```

Update module documentation to include `TraceId` in public surface.

### 1.3 Update lib.rs

Change:

```rust
pub use middleware::trace::TraceId;
```

To:

```rust
pub use domain::TraceId;
```

### 1.4 Update middleware/trace.rs

- Replace `TraceId` definition with `use crate::domain::TraceId;`
- Import task-local: `use crate::domain::trace_id::TRACE_ID;` (or re-export)
- Keep `Trace`, `TraceMiddleware` and Actix implementations
- Keep Actix-specific tests (lines 215-279)

### 1.5 Update domain imports

Change `use crate::middleware::trace::TraceId;` to `use crate::domain::TraceId;`
in:

- `backend/src/domain/error.rs:3`
- `backend/src/domain/user_events.rs:9`
- `backend/src/domain/user_onboarding.rs:9`
- `backend/src/domain/error/tests.rs:4`

### 1.6 Verify

```bash
make check-fmt && make lint && make test
```

---

## Step 2: Extract OpenAPI Schemas to Inbound Layer

Remove `utoipa::ToSchema` from domain types. Define schemas externally in the
inbound HTTP layer.

### 2.1 Create inbound/http/schemas.rs

**File**: `backend/src/inbound/http/schemas.rs`

Define OpenAPI schemas for domain types using utoipa's external schema
registration:

```rust
//! OpenAPI schema definitions for domain types.
//!
//! Domain types remain framework-agnostic. This module provides the
//! ToSchema implementations required for OpenAPI documentation.

use utoipa::ToSchema;

/// OpenAPI schema wrapper for [`crate::domain::ErrorCode`].
#[derive(ToSchema)]
#[schema(as = crate::domain::ErrorCode)]
pub enum ErrorCodeSchema {
    #[schema(rename = "invalid_request")]
    InvalidRequest,
    // ... match ErrorCode variants
}

/// OpenAPI schema wrapper for [`crate::domain::Error`].
#[derive(ToSchema)]
#[schema(as = crate::domain::Error)]
pub struct ErrorSchema {
    #[schema(example = "invalid_request")]
    code: ErrorCodeSchema,
    #[schema(example = "Something went wrong")]
    message: String,
    #[schema(example = "01HZY8B2W6X5Y7Z9ABCD1234")]
    trace_id: Option<String>,
    details: Option<serde_json::Value>,
}

/// OpenAPI schema wrapper for [`crate::domain::User`].
#[derive(ToSchema)]
#[schema(as = crate::domain::User)]
pub struct UserSchema {
    #[schema(value_type = String, example = "3fa85f64-5717-4562-b3fc-2c963f66afa6")]
    id: String,
    #[schema(value_type = String, example = "Ada Lovelace")]
    display_name: String,
}
```

### 2.2 Update inbound/http/mod.rs

Add:

```rust
pub mod schemas;
```

### 2.3 Remove ToSchema from domain/error.rs

- Remove `use utoipa::ToSchema;` (line 6)
- Remove `ToSchema` from `ErrorCode` derive (line 9)
- Remove `ToSchema` from `Error` derive (line 38)
- Remove all `#[schema(...)]` attributes (lines 43, 45, 48, 246)
- Remove `ToSchema` from `ErrorDto` derive (line 240)

### 2.4 Remove ToSchema from domain/user.rs

- Remove `use utoipa::ToSchema;` (line 8)
- Remove `ToSchema` from `User` derive (line 239)
- Remove all `#[schema(...)]` attributes (lines 244, 246)
- Remove `ToSchema` from `UserDto` derive (line 291)

### 2.5 Update doc.rs

Change:

```rust
components(schemas(User, Error, ErrorCode))
```

To:

```rust
components(schemas(
    crate::inbound::http::schemas::UserSchema,
    crate::inbound::http::schemas::ErrorSchema,
    crate::inbound::http::schemas::ErrorCodeSchema,
))
```

Update imports as needed.

### 2.6 Verify OpenAPI output

```bash
cargo run --bin openapi-dump > /tmp/openapi-after.json
# Compare with previous output to ensure schemas are preserved
```

### 2.7 Verify

```bash
make check-fmt && make lint && make test
```

---

## Step 3: Harden Architecture Lint

Add `utoipa` to forbidden crates for the domain layer.

### 3.1 Update architecture-lint/src/lib.rs

In `forbidden_crate_roots()` for `ModuleLayer::Domain`, add `"utoipa"`:

```rust
Self::Domain => BTreeSet::from([
    "actix",
    "actix_service",
    "actix_web",
    "actix_web_actors",
    "awc",
    "diesel",
    "diesel_async",
    "diesel_migrations",
    "pg_embedded_setup_unpriv",
    "postgres",
    "postgresql_embedded",
    "utoipa",  // Add this
]),
```

### 3.2 Add unit test

In `tools/architecture-lint/src/tests.rs`, add test case:

```rust
#[case(
    "domain/foo.rs",
    "use utoipa::ToSchema;",
    &["domain module must not depend on external crate `utoipa`"]
)]
```

### 3.3 Verify

```bash
make check-fmt && make lint && make test
```

---

## Step 4: Update Roadmap

Mark the task complete in `docs/backend-roadmap.md`:

```diff
- [ ] Update all modules to depend on the domain ports rather than reaching
+ [x] Update all modules to depend on the domain ports rather than reaching
```

---

## Testing Strategy

### Unit Tests (rstest)

1. **TraceId tests** moved to `domain/trace_id.rs`:
   - `trace_id_generate_produces_uuid`
   - `trace_id_current_reflects_scope`
   - `trace_id_current_is_none_out_of_scope`
   - `trace_id_from_str_round_trips`
   - `from_uuid_round_trips`

2. **Architecture lint tests** for utoipa violation detection

### Behavioural Tests (rstest-bdd)

Existing adapter guardrails tests in `backend/tests/adapter_guardrails/` should
continue passing without modification.

### Integration Tests

1. Verify `cargo run --bin openapi-dump` produces valid OpenAPI spec
2. Verify all domain types still serialize/deserialize correctly

---

## Files Modified

| File | Change |
|------|--------|
| `backend/src/domain/trace_id.rs` | **Create**: TraceId type + task-local |
| `backend/src/domain/mod.rs` | Add trace_id module export |
| `backend/src/domain/error.rs` | Remove utoipa imports/derives |
| `backend/src/domain/user.rs` | Remove utoipa imports/derives |
| `backend/src/domain/user_events.rs` | Update TraceId import |
| `backend/src/domain/user_onboarding.rs` | Update TraceId import |
| `backend/src/domain/error/tests.rs` | Update TraceId import |
| `backend/src/middleware/trace.rs` | Import TraceId from domain |
| `backend/src/lib.rs` | Update TraceId re-export |
| `backend/src/inbound/http/schemas.rs` | **Create**: OpenAPI schema wrappers |
| `backend/src/inbound/http/mod.rs` | Add schemas module |
| `backend/src/doc.rs` | Update schema references |
| `tools/architecture-lint/src/lib.rs` | Add utoipa to forbidden crates |
| `tools/architecture-lint/src/tests.rs` | Add utoipa violation test |
| `docs/backend-roadmap.md` | Mark task complete |

---

## Quality Gates

Before marking complete:

- [x] `make check-fmt` passes
- [x] `make lint` passes (includes architecture lint)
- [x] `make test` passes
- [x] OpenAPI dump produces valid, equivalent schemas
- [x] No domain code imports `utoipa` or `middleware`
