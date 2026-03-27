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

mod cursor;
mod envelope;
mod params;

pub use cursor::{Cursor, CursorError, Direction};
pub use envelope::{Paginated, PaginationLinks};
pub use params::{DEFAULT_LIMIT, MAX_LIMIT, PageParams, PageParamsError};
