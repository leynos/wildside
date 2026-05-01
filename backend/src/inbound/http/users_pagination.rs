//! Pagination helpers for the users HTTP adapter.
//!
//! The users endpoint owns HTTP query parsing, cursor decoding, link
//! construction, and OpenAPI response tokens. Domain ports receive decoded,
//! transport-neutral pagination requests.

use actix_web::HttpRequest;
use pagination::{Cursor, Direction, MAX_LIMIT, PageParams, Paginated, PaginationLinks};
use serde::Deserialize;
use serde_json::json;
use url::Url;
use utoipa::ToSchema;

use crate::domain::ports::{ListUsersPageRequest, UsersPage};
use crate::domain::{Error, User, UserCursorKey};
use crate::inbound::http::schemas::UserSchema;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsersPageDirection {
    First,
    Next,
    Prev,
}

/// Convert raw HTTP query parameters into pagination request objects.
///
/// # Errors
///
/// Returns an invalid request error when `limit` is malformed, zero, above
/// [`MAX_LIMIT`], or when `cursor` is not an opaque users cursor.
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
        .map_err(|_| invalid_cursor_error())?;
    let direction = cursor
        .as_ref()
        .map_or(UsersPageDirection::First, cursor_direction);
    let request = ListUsersPageRequest::new(cursor, page_params.limit());
    Ok((page_params, request, direction))
}

/// Build the paginated HTTP response envelope for one users page.
///
/// # Errors
///
/// Returns an internal error if cursor encoding or request URL reconstruction
/// fails.
pub fn build_users_page_response(
    request: &HttpRequest,
    params: &PageParams,
    page: UsersPage,
    direction: UsersPageDirection,
) -> Result<Paginated<User>, Error> {
    let has_more = page.has_more();
    let rows = page.into_rows();
    let next_cursor = next_cursor(&rows, has_more, direction)?;
    let prev_cursor = prev_cursor(&rows, has_more, direction)?;
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

fn next_cursor(
    rows: &[User],
    has_more: bool,
    direction: UsersPageDirection,
) -> Result<Option<String>, Error> {
    let should_emit = match direction {
        UsersPageDirection::First | UsersPageDirection::Next => has_more,
        UsersPageDirection::Prev => true,
    };
    encode_boundary_cursor(rows.last(), Direction::Next, should_emit)
}

fn prev_cursor(
    rows: &[User],
    has_more: bool,
    direction: UsersPageDirection,
) -> Result<Option<String>, Error> {
    let should_emit = match direction {
        UsersPageDirection::First => false,
        UsersPageDirection::Next => true,
        UsersPageDirection::Prev => has_more,
    };
    encode_boundary_cursor(rows.first(), Direction::Prev, should_emit)
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

fn invalid_cursor_error() -> Error {
    Error::invalid_request("cursor is invalid")
        .with_details(json!({ "field": "cursor", "code": "invalid_cursor" }))
}

fn invalid_limit_error(value: Option<&str>) -> Error {
    Error::invalid_request(format!("limit must be between 1 and {MAX_LIMIT}")).with_details(json!({
        "field": "limit",
        "code": "invalid_limit",
        "value": value,
        "max": MAX_LIMIT,
    }))
}
