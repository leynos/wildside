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
use crate::domain::ports::{ListUsersPageRequest, UsersPage};
use crate::domain::{Error, User, UserCursorKey};
use crate::inbound::http::schemas::UserSchema;
use crate::observability::pagination_errors::{PaginationErrorSource, record_pagination_error};

/// Raw users list query parameters.
///
/// Keeping `limit` as a string lets the handler return the shared API error
/// envelope for malformed or oversized values instead of Actix's extractor
/// error body.
#[derive(Debug, Clone, Deserialize)]
pub struct UsersListQueryParams {
    cursor: Option<String>,
    limit: Option<String>,
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

/// Direction implied by a users page request.
///
/// # Examples
///
/// ```
/// use backend::inbound::http::users_pagination::UsersPageDirection;
///
/// let direction = UsersPageDirection::Next;
/// let label = match direction {
///     UsersPageDirection::First => "first",
///     UsersPageDirection::Next => "next",
///     UsersPageDirection::Prev => "prev",
/// };
///
/// assert_eq!(label, "next");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsersPageDirection {
    /// The first page was requested without a cursor.
    First,
    /// A forward cursor was requested.
    Next,
    /// A backward cursor was requested.
    Prev,
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
/// use backend::domain::ports::ListUsersPageRequest;
/// use backend::inbound::http::users_pagination::{
///     PageParams, UsersListQueryParams, UsersPageDirection, parse_users_page_params,
/// };
///
/// let query = UsersListQueryParams {
///     cursor: None,
///     limit: Some("2".to_owned()),
/// };
/// let (params, request, direction): (
///     PageParams,
///     ListUsersPageRequest,
///     UsersPageDirection,
/// ) = parse_users_page_params(query).expect("valid users pagination params");
///
/// assert_eq!(params.limit(), 2);
/// assert_eq!(request.limit(), 2);
/// assert_eq!(direction, UsersPageDirection::First);
/// ```
pub fn parse_users_page_params(
    params: UsersListQueryParams,
) -> Result<(PageParams, ListUsersPageRequest, UsersPageDirection), Error> {
    let limit = parse_limit(params.limit.as_deref())?;
    let page_params = PageParams::new(params.cursor.clone(), limit)
        .map_err(|_| invalid_limit_error(params.limit.as_deref()))?;
    let cursor = params
        .cursor
        .as_deref()
        .map(Cursor::<UserCursorKey>::decode)
        .transpose()
        .map_err(map_cursor_error)?;
    let direction = cursor
        .as_ref()
        .map_or(UsersPageDirection::First, cursor_direction);
    let limit = NonZeroUsize::new(page_params.limit())
        .ok_or_else(|| invalid_limit_error(params.limit.as_deref()))?;
    let request = ListUsersPageRequest::new(cursor, limit);
    Ok((page_params, request, direction))
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
/// use backend::domain::User;
/// use backend::inbound::http::users_pagination::{
///     UsersPageDirection, build_users_page_response,
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
/// let response: Paginated<User> = build_users_page_response(
///     &request,
///     &params,
///     page,
///     UsersPageDirection::First,
/// )
/// .expect("users page response");
///
/// assert_eq!(response.data.len(), 1);
/// assert_eq!(response.limit, 2);
/// assert!(response.links.next.is_none());
/// assert!(response.links.prev.is_none());
/// ```
pub fn build_users_page_response(
    request: &HttpRequest,
    params: &PageParams,
    page: UsersPage,
    direction: UsersPageDirection,
) -> Result<Paginated<User>, Error> {
    let has_more = page.has_more();
    let rows = page.into_rows();
    let next_cursor = boundary_cursor(rows.last(), Direction::Next, direction, has_more)?;
    let prev_cursor = boundary_cursor(rows.first(), Direction::Prev, direction, has_more)?;
    let request_url = current_request_url(request)?;
    let links = PaginationLinks::from_request(
        &request_url,
        params,
        next_cursor.as_deref(),
        prev_cursor.as_deref(),
    );

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

fn cursor_direction(cursor: &Cursor<UserCursorKey>) -> UsersPageDirection {
    match cursor.direction() {
        Direction::Next => UsersPageDirection::Next,
        Direction::Prev => UsersPageDirection::Prev,
    }
}

fn boundary_cursor(
    row: Option<&User>,
    cursor_dir: Direction,
    page_dir: UsersPageDirection,
    has_more: bool,
) -> Result<Option<String>, Error> {
    let should_emit = match (page_dir, cursor_dir) {
        (UsersPageDirection::First, Direction::Next) => has_more,
        (UsersPageDirection::First, Direction::Prev) => false,
        (UsersPageDirection::Next, Direction::Next) => has_more,
        (UsersPageDirection::Next, Direction::Prev) => true,
        (UsersPageDirection::Prev, Direction::Next) => true,
        (UsersPageDirection::Prev, Direction::Prev) => has_more,
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

fn map_cursor_error(error: CursorError) -> Error {
    match error {
        CursorError::UnsupportedDirection { direction } => {
            debug!(
                rejected_direction = %direction,
                source = "users_http",
                "rejected users cursor with unsupported direction"
            );
            record_pagination_error(PaginationErrorSource::UsersHttp, "unsupported_direction");
            unsupported_direction_error()
        }
        CursorError::InvalidBase64 { message } | CursorError::Deserialize { message } => {
            debug!(
                cursor_decode_error = %message,
                source = "users_http",
                "rejected users cursor with decode failure"
            );
            record_pagination_error(PaginationErrorSource::UsersHttp, "invalid_cursor");
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
