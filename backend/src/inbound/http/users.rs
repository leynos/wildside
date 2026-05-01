//! Users API handlers.
//!
//! ```text
//! POST /api/v1/login {"username":"admin","password":"password"}
//! GET /api/v1/users
//! GET /api/v1/users/me
//! PUT /api/v1/users/me/interests
//! ```

use crate::domain::ports::UpdateUserInterestsRequest;
use crate::domain::{
    Error, InterestThemeId, InterestThemeIdValidationError, LoginCredentials, LoginValidationError,
    User, UserInterests,
};
use crate::inbound::http::ApiResult;
use crate::inbound::http::schemas::{ErrorSchema, UserInterestsSchema, UserSchema};
use crate::inbound::http::session::SessionContext;
use crate::inbound::http::state::HttpState;
use crate::inbound::http::users_pagination::{
    PaginatedUsersResponse, UsersListQueryParams, build_users_page_response,
    parse_users_page_params,
};
use actix_web::{HttpRequest, HttpResponse, get, post, put, web};
use pagination::Paginated;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Login request body for `POST /api/v1/login`.
///
/// Example JSON:
/// `{"username":"admin","password":"password"}`
#[derive(Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// Interest theme update payload for `PUT /api/v1/users/me/interests`.
#[derive(Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct InterestsRequest {
    // The #[schema(max_items = 100)] must equal INTEREST_THEME_IDS_MAX.
    #[schema(max_items = 100)]
    pub interest_theme_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_revision: Option<u32>,
}

// This constant must match the #[schema(max_items = 100)] on
// InterestsRequest::interest_theme_ids.
/// Maximum interest theme IDs per user; prevents payload bloat and ensures
/// reasonable UI rendering.
const INTEREST_THEME_IDS_MAX: usize = 100;
#[derive(Debug)]
enum InterestsRequestError {
    TooManyInterestThemeIds {
        length: usize,
        max: usize,
    },
    InvalidInterestThemeId {
        index: usize,
        value: String,
        error: InterestThemeIdValidationError,
    },
}

fn parse_interest_theme_ids(
    payload: InterestsRequest,
) -> Result<ParsedInterests, InterestsRequestError> {
    if payload.interest_theme_ids.len() > INTEREST_THEME_IDS_MAX {
        return Err(InterestsRequestError::TooManyInterestThemeIds {
            length: payload.interest_theme_ids.len(),
            max: INTEREST_THEME_IDS_MAX,
        });
    }

    let interest_theme_ids = payload
        .interest_theme_ids
        .into_iter()
        .enumerate()
        .map(|(index, value)| {
            InterestThemeId::new(&value).map_err(|error| {
                InterestsRequestError::InvalidInterestThemeId {
                    index,
                    value,
                    error,
                }
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ParsedInterests {
        interest_theme_ids,
        expected_revision: payload.expected_revision,
    })
}

#[derive(Debug)]
struct ParsedInterests {
    interest_theme_ids: Vec<InterestThemeId>,
    expected_revision: Option<u32>,
}

impl TryFrom<LoginRequest> for LoginCredentials {
    type Error = LoginValidationError;

    fn try_from(value: LoginRequest) -> Result<Self, Self::Error> {
        Self::try_from_parts(&value.username, &value.password)
    }
}

/// Authenticate user and establish a session.
///
/// Uses the centralised `Error` type so clients get a consistent
/// error schema across all endpoints.
#[utoipa::path(
    post,
    path = "/api/v1/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login success", headers(("Set-Cookie" = String, description = "Session cookie"))),
        (status = 400, description = "Invalid request", body = ErrorSchema),
        (status = 401, description = "Invalid credentials", body = ErrorSchema),
        (status = 500, description = "Internal server error")
    ),
    tags = ["users"],
    operation_id = "login",
    security([])
)]
#[post("/login")]
pub async fn login(
    state: web::Data<HttpState>,
    session: SessionContext,
    payload: web::Json<LoginRequest>,
) -> ApiResult<HttpResponse> {
    let credentials =
        LoginCredentials::try_from(payload.into_inner()).map_err(map_login_validation_error)?;
    let is_admin = credentials.username() == "admin";
    let user_id = state.login.authenticate(&credentials).await?;
    session.persist_authenticated_user(&user_id, is_admin)?;
    Ok(HttpResponse::Ok().finish())
}

fn map_login_validation_error(err: LoginValidationError) -> Error {
    match err {
        LoginValidationError::EmptyUsername => Error::invalid_request("username must not be empty")
            .with_details(json!({ "field": "username", "code": "empty_username" })),
        LoginValidationError::EmptyPassword => Error::invalid_request("password must not be empty")
            .with_details(json!({ "field": "password", "code": "empty_password" })),
    }
}

fn map_interests_request_error(err: InterestsRequestError) -> Error {
    match err {
        InterestsRequestError::TooManyInterestThemeIds { length, max } => Error::invalid_request(
            format!("interest theme ids must contain at most {max} items"),
        )
        .with_details(json!({
            "field": "interestThemeIds",
            "code": "too_many_interest_theme_ids",
            "count": length,
            "max": max,
        })),
        InterestsRequestError::InvalidInterestThemeId {
            index,
            value,
            error,
        } => {
            let (message, code) = match error {
                InterestThemeIdValidationError::EmptyId => (
                    "interest theme id must not be empty",
                    "empty_interest_theme_id",
                ),
                InterestThemeIdValidationError::InvalidId => (
                    "interest theme id must be a valid UUID",
                    "invalid_interest_theme_id",
                ),
            };
            Error::invalid_request(message).with_details(json!({
                "field": "interestThemeIds",
                "index": index,
                "value": value,
                "code": code,
            }))
        }
    }
}

/// List known users.
///
/// # Examples
/// ```
/// use actix_web::App;
/// use backend::inbound::http::users::list_users;
///
/// let app = App::new().service(list_users);
/// ```
#[utoipa::path(
    get,
    path = "/api/v1/users",
    params(
        ("cursor" = Option<String>, Query, description = "Opaque users pagination cursor"),
        ("limit" = Option<usize>, Query, description = "Number of users to return, default 20, max 100")
    ),
    responses(
        (status = 200, description = "Users", body = PaginatedUsersResponse),
        (status = 400, description = "Invalid request", body = ErrorSchema),
        (status = 401, description = "Unauthorised", body = ErrorSchema),
        (status = 403, description = "Forbidden", body = ErrorSchema),
        (status = 404, description = "Not found", body = ErrorSchema),
        (status = 500, description = "Internal server error", body = ErrorSchema)
    ),
    tags = ["users"],
    operation_id = "listUsers",
    security(("SessionCookie" = []))
)]
#[get("/users")]
pub async fn list_users(
    state: web::Data<HttpState>,
    session: SessionContext,
    request: HttpRequest,
    params: web::Query<UsersListQueryParams>,
) -> ApiResult<web::Json<Paginated<User>>> {
    let user_id = session.require_user_id()?;
    let (page_params, page_request, direction) = parse_users_page_params(params.into_inner())?;
    let page = state.users.list_users_page(&user_id, page_request).await?;
    let response = build_users_page_response(&request, &page_params, page, direction)?;
    Ok(web::Json(response))
}

/// Fetch the authenticated user's profile.
#[utoipa::path(
    get,
    path = "/api/v1/users/me",
    responses(
        (status = 200, description = "User profile", body = UserSchema),
        (status = 401, description = "Unauthorised", body = ErrorSchema),
        (status = 500, description = "Internal server error", body = ErrorSchema)
    ),
    tags = ["users"],
    operation_id = "currentUser",
    security(("SessionCookie" = []))
)]
#[get("/users/me")]
pub async fn current_user(
    state: web::Data<HttpState>,
    session: SessionContext,
) -> ApiResult<web::Json<User>> {
    let user_id = session.require_user_id()?;
    let user = state.profile.fetch_profile(&user_id).await?;
    Ok(web::Json(user))
}

/// Update the authenticated user's interest theme selections.
#[utoipa::path(
    put,
    path = "/api/v1/users/me/interests",
    request_body = InterestsRequest,
    responses(
        (status = 200, description = "Updated interests", body = UserInterestsSchema),
        (status = 400, description = "Invalid request", body = ErrorSchema),
        (status = 401, description = "Unauthorised", body = ErrorSchema),
        (status = 409, description = "Conflict", body = ErrorSchema),
        (status = 500, description = "Internal server error", body = ErrorSchema)
    ),
    tags = ["users"],
    operation_id = "updateUserInterests",
    security(("SessionCookie" = []))
)]
#[put("/users/me/interests")]
pub async fn update_interests(
    state: web::Data<HttpState>,
    session: SessionContext,
    payload: web::Json<InterestsRequest>,
) -> ApiResult<web::Json<UserInterests>> {
    let user_id = session.require_user_id()?;
    let parsed =
        parse_interest_theme_ids(payload.into_inner()).map_err(map_interests_request_error)?;
    let interests = state
        .interests
        .set_interests(UpdateUserInterestsRequest {
            user_id,
            interest_theme_ids: parsed.interest_theme_ids,
            expected_revision: parsed.expected_revision,
        })
        .await?;
    Ok(web::Json(interests))
}

#[cfg(test)]
mod tests;
