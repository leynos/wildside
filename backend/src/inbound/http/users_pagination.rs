//! Pagination helpers for the users HTTP adapter.
//!
//! The users endpoint owns HTTP query parsing, cursor decoding, link
//! construction, and OpenAPI response tokens. Domain ports receive decoded,
//! transport-neutral pagination requests.

use std::num::NonZeroUsize;

use actix_web::HttpRequest;
use pagination::{
    Cursor, CursorError, Direction, MAX_LIMIT, PageParams, Paginated, PaginationLinks,
};
use serde::Deserialize;
use serde_json::json;
use tracing::debug;
use url::Url;
use utoipa::ToSchema;

use crate::domain::pagination_errors::{invalid_cursor_error, unsupported_direction_error};
use crate::domain::ports::{
    ListUsersPageRequest, PageTraversal, PaginationErrorMetrics, PaginationErrorSource,
};
use crate::domain::{Error, User, UserCursorKey};
use crate::inbound::http::schemas::UserSchema;

mod telemetry;
pub use telemetry::UsersPageResponse;

/// Raw users list query parameters.
///
/// Keeping `limit` as a string lets the handler return the shared API error
/// envelope for malformed or oversized values instead of Actix's extractor
/// error body.
#[derive(Debug, Clone, Deserialize)]
pub struct UsersListQueryParams {
    cursor: Option<String>,
    pub(super) limit: Option<String>,
}

/// OpenAPI schema for pagination links in `GET /api/v1/users` responses.
#[derive(ToSchema)]
#[expect(
    dead_code,
    reason = "Used only for OpenAPI schema generation via utoipa"
)]
pub struct PaginationLinksSchema {
    /// Canonical URL for the current page.
    #[schema(
        rename = "self",
        example = "https://example.test/api/v1/users?limit=20"
    )]
    self_: String,
    /// URL for the next page, when a following page exists.
    #[schema(example = "https://example.test/api/v1/users?limit=20&cursor=opaque")]
    next: Option<String>,
    /// URL for the previous page, when an earlier page exists.
    #[schema(example = "https://example.test/api/v1/users?limit=20&cursor=opaque")]
    prev: Option<String>,
}

/// OpenAPI schema for the paginated users response envelope.
#[derive(ToSchema)]
#[expect(
    dead_code,
    reason = "Used only for OpenAPI schema generation via utoipa"
)]
pub struct PaginatedUsersResponse {
    /// Users in stable `(createdAt, id)` order.
    data: Vec<UserSchema>,
    /// Effective page size for this response.
    #[schema(minimum = 1, maximum = 100, example = 20)]
    limit: usize,
    /// Hypermedia links for page navigation.
    links: PaginationLinksSchema,
}

/// Convert raw HTTP query parameters into pagination request objects.
///
/// # Errors
///
/// Returns an invalid request error when `limit` is malformed, zero, above
/// [`MAX_LIMIT`], or when `cursor` is not an opaque users cursor.
///
/// # Examples
///
/// ```ignore
/// use std::num::NonZeroUsize;
/// use backend::domain::ports::ListUsersPageRequest;
/// use backend::domain::ports::NoOpPaginationErrorMetrics;
/// use backend::inbound::http::users_pagination::{
///     PageParams, UsersListQueryParams, parse_users_page_params,
/// };
/// use backend::domain::ports::PageTraversal;
///
/// let query = UsersListQueryParams {
///     cursor: None,
///     limit: Some("2".to_owned()),
/// };
/// let (params, request, direction, page_limit): (
///     PageParams,
///     ListUsersPageRequest,
///     PageTraversal,
///     NonZeroUsize,
/// ) = parse_users_page_params(query, &NoOpPaginationErrorMetrics)
///     .expect("valid users pagination params");
///
/// assert_eq!(params.limit(), 2, "the parsed page limit must be retained");
/// assert_eq!(request.limit(), 2, "the query request uses the parsed limit");
/// assert_eq!(direction, PageTraversal::First, "a missing cursor starts a traversal");
/// assert_eq!(page_limit.get(), 2, "the validated limit remains non-zero");
/// ```
pub fn parse_users_page_params(
    params: UsersListQueryParams,
    pagination_error_metrics: &dyn PaginationErrorMetrics,
) -> Result<
    (
        PageParams,
        ListUsersPageRequest,
        PageTraversal,
        NonZeroUsize,
    ),
    Error,
> {
    let limit = parse_limit(params.limit.as_deref())?;
    let page_params = PageParams::new(params.cursor.clone(), limit)
        .map_err(|_| invalid_limit_error(params.limit.as_deref()))?;
    let cursor = params
        .cursor
        .as_deref()
        .map(Cursor::<UserCursorKey>::decode)
        .transpose()
        .map_err(|error| map_cursor_error(error, pagination_error_metrics))?;
    let direction = cursor
        .as_ref()
        .map_or(PageTraversal::First, cursor_direction);
    let limit = NonZeroUsize::new(page_params.limit())
        .ok_or_else(|| invalid_limit_error(params.limit.as_deref()))?;
    let request = ListUsersPageRequest::new(cursor, limit);
    Ok((page_params, request, direction, limit))
}

/// Build the paginated HTTP response envelope for one users page.
///
/// # Errors
///
/// Returns an internal error if cursor encoding or request URL reconstruction
/// fails.
///
/// # Examples
///
/// ```
/// use actix_web::test::TestRequest;
/// use backend::domain::ports::UsersPage;
/// use backend::domain::ports::PageTraversal;
/// use backend::domain::User;
/// use backend::domain::ports::{LimitSource, NoOpPaginationPageMetrics};
/// use backend::inbound::http::users_pagination::{
///     UsersPageResponse, build_users_page_response,
/// };
/// use pagination::{PageParams, Paginated};
///
/// let request = TestRequest::default()
///     .uri("/api/v1/users?limit=2")
///     .to_http_request();
/// let params = PageParams::new(None, Some(2)).expect("valid page params");
/// let user = User::try_from_strings_at(
///     "11111111-1111-1111-1111-111111111111",
///     "Ada One",
///     "2026-01-01T00:00:00Z".parse().expect("timestamp"),
/// )
/// .expect("valid user");
/// let page = UsersPage::new(vec![user], false);
///
/// let response: Paginated<User> = build_users_page_response(UsersPageResponse {
///     request: &request,
///     params: &params,
///     page,
///     traversal: PageTraversal::First,
///     limit_source: LimitSource::Explicit,
///     page_limit: NonZeroUsize::new(2).expect("the example limit is non-zero"),
///     metrics: &NoOpPaginationPageMetrics,
/// })
/// .expect("users page response");
///
/// assert_eq!(response.data.len(), 1, "the response carries the returned row");
/// assert_eq!(response.limit, 2, "the response retains its effective limit");
/// assert!(response.links.next.is_none(), "a short first page has no next link");
/// assert!(response.links.prev.is_none(), "the first page has no previous link");
/// ```
pub fn build_users_page_response(input: UsersPageResponse<'_>) -> Result<Paginated<User>, Error> {
    let UsersPageResponse {
        request,
        params,
        page,
        traversal,
        limit_source,
        page_limit,
        metrics,
    } = input;
    let has_more = page.has_more();
    let rows = page.into_rows();
    let next_cursor = boundary_cursor(rows.last(), Direction::Next, traversal, has_more)?;
    let prev_cursor = boundary_cursor(rows.first(), Direction::Prev, traversal, has_more)?;
    let request_url = current_request_url(request)?;
    let links = PaginationLinks::from_request(
        &request_url,
        params,
        next_cursor.as_deref(),
        prev_cursor.as_deref(),
    );

    telemetry::record_page_delivery(&telemetry::UsersPageTelemetry {
        metrics,
        traversal,
        limit_source,
        page_limit,
        returned_rows: rows.len(),
        has_next_link: next_cursor.is_some(),
        has_prev_link: prev_cursor.is_some(),
    });

    Ok(Paginated::new(rows, params.limit(), links))
}

fn parse_limit(raw: Option<&str>) -> Result<Option<usize>, Error> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let value = raw
        .parse::<usize>()
        .map_err(|_| invalid_limit_error(Some(raw)))?;
    if value == 0 || value > MAX_LIMIT {
        return Err(invalid_limit_error(Some(raw)));
    }
    Ok(Some(value))
}

fn cursor_direction(cursor: &Cursor<UserCursorKey>) -> PageTraversal {
    match cursor.direction() {
        Direction::Next => PageTraversal::Next,
        Direction::Prev => PageTraversal::Prev,
    }
}

fn boundary_cursor(
    row: Option<&User>,
    cursor_dir: Direction,
    page_dir: PageTraversal,
    has_more: bool,
) -> Result<Option<String>, Error> {
    let should_emit = match (page_dir, cursor_dir) {
        (PageTraversal::First, Direction::Next) => has_more,
        (PageTraversal::First, Direction::Prev) => false,
        (PageTraversal::Next, Direction::Next) => has_more,
        (PageTraversal::Next, Direction::Prev) => true,
        (PageTraversal::Prev, Direction::Next) => true,
        (PageTraversal::Prev, Direction::Prev) => has_more,
    };
    encode_boundary_cursor(row, cursor_dir, should_emit)
}

fn encode_boundary_cursor(
    user: Option<&User>,
    direction: Direction,
    should_emit: bool,
) -> Result<Option<String>, Error> {
    if !should_emit {
        return Ok(None);
    }
    let Some(user) = user else {
        return Ok(None);
    };
    Cursor::with_direction(UserCursorKey::from(user), direction)
        .encode()
        .map(Some)
        .map_err(|err| Error::internal(format!("failed to encode users cursor: {err}")))
}

fn current_request_url(request: &HttpRequest) -> Result<Url, Error> {
    let connection = request.connection_info();
    let url = format!(
        "{}://{}{}",
        connection.scheme(),
        connection.host(),
        request.uri()
    );
    Url::parse(&url).map_err(|err| Error::internal(format!("failed to build request URL: {err}")))
}

/// Log the cursor rejection, record it through the injected metrics port, and
/// map it to the matching client-facing error.
fn map_cursor_error(
    error: CursorError,
    pagination_error_metrics: &dyn PaginationErrorMetrics,
) -> Error {
    match error {
        CursorError::UnsupportedDirection { direction } => {
            debug!(
                rejected_direction = %direction,
                source = "users_http",
                "rejected users cursor with unsupported direction"
            );
            pagination_error_metrics
                .record(PaginationErrorSource::UsersHttp, "unsupported_direction");
            unsupported_direction_error()
        }
        CursorError::InvalidBase64 { message } | CursorError::Deserialize { message } => {
            debug!(
                cursor_decode_error = %message,
                source = "users_http",
                "rejected users cursor with decode failure"
            );
            pagination_error_metrics.record(PaginationErrorSource::UsersHttp, "invalid_cursor");
            invalid_cursor_error()
        }
        CursorError::Serialize { message } => {
            debug!(
                cursor_serialize_error = %message,
                source = "users_http",
                "users cursor decode raised serialize error"
            );
            Error::internal("failed to decode users cursor")
        }
    }
}

fn invalid_limit_error(value: Option<&str>) -> Error {
    Error::invalid_request(format!("limit must be between 1 and {MAX_LIMIT}")).with_details(json!({
        "field": "limit",
        "code": "invalid_limit",
        "value": value,
        "max": MAX_LIMIT,
    }))
}

#[cfg(test)]
mod tests {
    //! Verify that HTTP cursor errors use the injected metrics recorder.

    use std::sync::Mutex;

    use crate::domain::ErrorCode;
    use crate::domain::ports::{PaginationErrorMetrics, PaginationErrorSource};
    use rstest::rstest;

    use super::{UsersListQueryParams, parse_users_page_params};

    #[derive(Default)]
    struct RecordingPaginationErrorMetrics(Mutex<Vec<(PaginationErrorSource, &'static str)>>);

    impl PaginationErrorMetrics for RecordingPaginationErrorMetrics {
        fn increment(&self, source: PaginationErrorSource, detail_code: &'static str) {
            self.0
                .lock()
                .expect("pagination metrics lock should not be poisoned")
                .push((source, detail_code));
        }
    }

    /// Parse a cursor and assert the HTTP adapter recorded the expected code.
    fn assert_cursor_error_is_recorded(cursor: &str, expected_detail_code: &'static str) {
        let metrics = RecordingPaginationErrorMetrics::default();
        let params = UsersListQueryParams {
            cursor: Some(cursor.to_owned()),
            limit: None,
        };

        let error = match parse_users_page_params(params, &metrics) {
            Err(error) => error,
            Ok(_) => panic!("invalid cursor should be rejected"),
        };

        assert_eq!(error.code(), ErrorCode::InvalidRequest);
        assert_eq!(
            *metrics
                .0
                .lock()
                .expect("pagination metrics lock should not be poisoned"),
            vec![(PaginationErrorSource::UsersHttp, expected_detail_code)]
        );
    }

    // This opaque fixture decodes to a users cursor with `dir: "Sideways"`.
    const UNSUPPORTED_DIRECTION_CURSOR: &str = "eyJrZXkiOnsiY3JlYXRlZF9hdCI6IjIwMjYtMDMtMjJUMTA6MzA6MDBaIiwiaWQiOiIxMTExMTExMS0xMTExLTExMTEtMTExMS0xMTExMTExMTExMTEifSwiZGlyIjoiU2lkZXdheXMifQ";

    /// Assert each rejected cursor records the expected detail code.
    #[rstest]
    #[case::invalid_cursor("not-a-cursor", "invalid_cursor")]
    #[case::unsupported_direction(UNSUPPORTED_DIRECTION_CURSOR, "unsupported_direction")]
    fn records_cursor_error_metrics(#[case] cursor: &str, #[case] expected_code: &'static str) {
        assert_cursor_error_is_recorded(cursor, expected_code);
    }
}
