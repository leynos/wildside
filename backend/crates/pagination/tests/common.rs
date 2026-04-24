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

/// Helper to create and store page parameters with the given limit.
///
/// This helper is shared by BDD step definitions that need to set up
/// page parameters with a specific limit value.
///
/// # Panics
///
/// Panics in either of the following cases:
///
/// - If `usize::try_from(limit)` fails (i.e. `limit` exceeds `usize::MAX`),
///   with the message `"fixture limit should fit usize"`.
/// - If `PageParams::new(None, Some(requested_limit))` returns `Err` (for
///   example, when `requested_limit` is zero or otherwise invalid), with the
///   message `"params should be valid"`.
#[expect(
    clippy::expect_used,
    reason = "BDD helpers use expect for clear failures"
)]
pub fn set_page_params_with_limit(world: &World, limit: u64) {
    let requested_limit = usize::try_from(limit).expect("fixture limit should fit usize");
    let params = PageParams::new(None, Some(requested_limit)).expect("params should be valid");
    world.page_params.set(params);
}
