//! Shared opaque cursor and pagination envelope primitives.
//!
//! This crate stays transport- and persistence-neutral so inbound HTTP
//! handlers and outbound repositories can share one cursor contract without
//! coupling the pagination model to Actix, Diesel, or endpoint-specific
//! schemas.
//!
//! # Cursor-based Pagination
//!
//! Cursors are opaque tokens that encode a position within an ordered dataset.
//! They support bidirectional navigation via [`Direction`] (`Next` or `Prev`).
//!
//! # Example
//!
//! ```
//! use pagination::{Cursor, Direction, PageParams, Paginated, PaginationLinks};
//! use serde::{Deserialize, Serialize};
//! use url::Url;
//!
//! #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
//! struct UserKey {
//!     created_at: String,
//!     id: String,
//! }
//!
//! let params = PageParams::new(None, Some(25)).expect("valid page params");
//! let current_url = Url::parse("https://example.test/api/v1/users").expect("valid url");
//! let next_cursor = Cursor::with_direction(
//!     UserKey {
//!         created_at: "2026-03-22T10:30:00Z".to_owned(),
//!         id: "8b116c56-0a58-4c55-b7d7-06ee6bbddb8c".to_owned(),
//!     },
//!     Direction::Next,
//! )
//! .encode()
//! .expect("cursor encoding succeeds");
//!
//! let page = Paginated::new(
//!     vec!["Ada Lovelace"],
//!     params.limit(),
//!     PaginationLinks::from_request(&current_url, &params, Some(next_cursor.as_str()), None),
//! );
//!
//! assert_eq!(page.limit, 25);
//! assert!(page.links.next.is_some());
//! ```
//!
//! # Ordering Requirements
//!
//! Key types used with [`Cursor`] must satisfy strict ordering guarantees to
//! ensure stable, deterministic pagination across all pages of a given endpoint:
//!
//! - **Serialization bounds**: the key type must implement `Serialize` and
//!   `DeserializeOwned` so cursors can be encoded to and decoded from opaque
//!   base64url-encoded JSON tokens.
//! - **Total ordering**: the key fields must correspond to a composite database
//!   index that provides a total ordering over the result set. This typically
//!   means ordering by one or more filterable fields (e.g., `created_at`,
//!   `updated_at`, `name`) followed by a unique field (typically a UUID primary
//!   key) to break ties.
//! - **Stability**: the ordering must remain consistent across all pages of a
//!   single endpoint. If the ordering changes between requests (for example, if
//!   the database index is modified or if the key fields do not correspond to a
//!   stable index), cursors may skip or duplicate records.
//!
//! ```no_run
//! # use serde::{Deserialize, Serialize};
//! #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
//! struct OrderKey {
//!     created_at: String,  // ORDER BY created_at DESC
//!     id: String,          // THEN BY id DESC (tie-breaker)
//! }
//! ```
//!
//! Consumers are responsible for ensuring that database queries use the
//! appropriate index and ordering clauses. The pagination crate does not
//! validate ordering correctness at runtime.
//!
//! # Default and Maximum Limits
//!
//! Page size limits are controlled by two shared constants:
//!
//! - [`DEFAULT_LIMIT`]: 20 records per page (applied when no limit is provided)
//! - [`MAX_LIMIT`]: 100 records per page (upper bound for all requests)
//!
//! The [`PageParams`] type automatically normalizes limit values during
//! construction and deserialization:
//!
//! - If no limit is provided (`None`), the default limit (20) is applied.
//! - If the limit is provided but exceeds the maximum (100), it is capped at
//!   the maximum.
//! - If the limit is zero, [`PageParams::new`] returns
//!   `Err(PageParamsError::InvalidLimit)`.
//!
//! ```no_run
//! # use pagination::{PageParams, DEFAULT_LIMIT, MAX_LIMIT};
//! let default_page = PageParams::new(None, None).unwrap();
//! assert_eq!(default_page.limit(), DEFAULT_LIMIT);
//!
//! let capped_page = PageParams::new(None, Some(200)).unwrap();
//! assert_eq!(capped_page.limit(), MAX_LIMIT);
//!
//! let zero_limit = PageParams::new(None, Some(0));
//! assert!(zero_limit.is_err());
//! ```
//!
//! Consumers should use these constants when constructing responses and
//! validating incoming page parameters.
//!
//! # Error Mapping Guidelines
//!
//! The pagination crate defines two error types:
//!
//! - [`CursorError`]: raised when cursor encoding or decoding fails
//! - [`PageParamsError`]: raised when page parameters are invalid
//!
//! Consumers (typically inbound HTTP adapters) should map these errors to HTTP
//! responses as follows:
//!
//! | Error Type          | Variant          | HTTP Status     | Envelope `code`         |
//! |---------------------|------------------|-----------------|-------------------------|
//! | [`CursorError`]     | `InvalidBase64`  | 400 Bad Request | `invalid_cursor`        |
//! | [`CursorError`]     | `Deserialize`    | 400 Bad Request | `invalid_cursor`        |
//! | [`CursorError`]     | `Serialize`      | 500 Internal    | `internal_error`        |
//! | [`PageParamsError`] | `InvalidLimit`   | 400 Bad Request | `invalid_page_params`   |
//!
//! Note that `CursorError::Serialize` maps to HTTP 500 (not 400) because it
//! indicates a bug in the server (the key type could not be serialized), not a
//! client error. Consumers should log serialization failures and investigate the
//! root cause.
//!
//! Example error response envelope:
//!
//! ```no_run
//! # use serde_json::json;
//! let error_response = json!({
//!     "code": "invalid_cursor",
//!     "message": "The cursor parameter is malformed",
//! });
//! ```
//!
//! # Scope Boundaries
//!
//! This crate intentionally does **not** provide:
//!
//! - **Diesel query filters**: consumers must build their own `WHERE` clauses
//!   and apply cursor key conditions to the query. See
//!   `docs/keyset-pagination-design.md` for integration patterns.
//! - **Actix extractors**: consumers must deserialize [`PageParams`] manually
//!   using `Query<PageParams>` or similar extractors provided by the framework.
//! - **Connection pooling**: consumers must manage database connections and pass
//!   them to repository methods.
//! - **`OpenAPI` schema generation**: consumers must define schema annotations for
//!   their endpoint-specific key types and response envelopes.
//!
//! These responsibilities belong to the inbound and outbound adapters that
//! consume the pagination primitives. Keeping the crate transport- and
//! persistence-neutral ensures it can be reused across multiple HTTP frameworks
//! and database libraries.

mod cursor;
mod envelope;
mod params;

pub use cursor::{Cursor, CursorError, Direction};
pub use envelope::{Paginated, PaginationLinks};
pub use params::{DEFAULT_LIMIT, MAX_LIMIT, PageParams, PageParamsError};
