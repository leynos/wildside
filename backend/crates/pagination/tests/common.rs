//! Common fixtures and types shared across BDD test modules.

use pagination::Paginated;
use rstest_bdd::Slot;
use rstest_bdd_macros::ScenarioState;
use serde::{Deserialize, Serialize};
use url::Url;

// Re-export pagination types for use in test modules
pub use pagination::{Cursor, CursorError, Direction, PageParams, PageParamsError};

/// Fixture key type used for testing cursor encoding and decoding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixtureKey {
    /// Timestamp field for ordering.
    pub created_at: String,
    /// Unique identifier.
    pub id: String,
}

/// BDD test world state container.
#[derive(Debug, Default, ScenarioState)]
pub struct World {
    /// The ordering key fixture.
    pub key: Slot<FixtureKey>,
    /// Cursor token string.
    pub cursor_token: Slot<String>,
    /// Result of cursor decoding.
    pub decode_result: Slot<Result<Cursor<FixtureKey>, CursorError>>,
    /// Page parameters.
    pub page_params: Slot<PageParams>,
    /// Result of page parameter creation.
    pub page_params_result: Slot<Result<PageParams, PageParamsError>>,
    /// Request URL with query parameters.
    pub request_url: Slot<Url>,
    /// Paginated response envelope.
    pub page: Slot<Paginated<String>>,
    /// Pagination direction.
    pub direction: Slot<Direction>,
    /// Collection of cursor errors for testing.
    pub cursor_errors: Slot<Vec<CursorError>>,
}
