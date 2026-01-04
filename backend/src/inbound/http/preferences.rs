//! User preferences HTTP handlers.
//!
//! ```text
//! GET /api/v1/users/me/preferences
//! PUT /api/v1/users/me/preferences
//! ```

use std::str::FromStr;

use actix_web::{HttpRequest, HttpResponse, get, put, web};
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::domain::ports::UpdatePreferencesRequest;
use crate::domain::{Error, UnitSystem, UserPreferences};
use crate::inbound::http::ApiResult;
use crate::inbound::http::idempotency::{extract_idempotency_key, map_idempotency_key_error};
use crate::inbound::http::schemas::ErrorSchema;
use crate::inbound::http::session::SessionContext;
use crate::inbound::http::state::HttpState;
use crate::inbound::http::validation::{missing_field_error, parse_uuid_list};

/// Request payload for updating user preferences.
#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PreferencesRequest {
    pub interest_theme_ids: Option<Vec<String>>,
    pub safety_toggle_ids: Option<Vec<String>>,
    pub unit_system: Option<String>,
    pub expected_revision: Option<u32>,
}

/// Response payload for user preferences.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserPreferencesResponse {
    pub user_id: String,
    pub interest_theme_ids: Vec<String>,
    pub safety_toggle_ids: Vec<String>,
    pub unit_system: String,
    pub revision: u32,
    pub updated_at: String,
}

impl From<UserPreferences> for UserPreferencesResponse {
    fn from(value: UserPreferences) -> Self {
        Self {
            user_id: value.user_id.to_string(),
            interest_theme_ids: value
                .interest_theme_ids
                .into_iter()
                .map(|id| id.to_string())
                .collect(),
            safety_toggle_ids: value
                .safety_toggle_ids
                .into_iter()
                .map(|id| id.to_string())
                .collect(),
            unit_system: value.unit_system.to_string(),
            revision: value.revision,
            updated_at: value.updated_at.to_rfc3339(),
        }
    }
}

fn invalid_unit_system_error(value: &str) -> Error {
    Error::invalid_request("unit system must be metric or imperial").with_details(json!({
        "field": "unitSystem",
        "value": value,
        "code": "invalid_unit_system",
    }))
}

fn parse_unit_system(value: String) -> Result<UnitSystem, Error> {
    UnitSystem::from_str(&value).map_err(|_| invalid_unit_system_error(&value))
}

fn parse_preferences_request(payload: PreferencesRequest) -> Result<ParsedPreferences, Error> {
    let interest_theme_ids = payload
        .interest_theme_ids
        .ok_or_else(|| missing_field_error("interestThemeIds"))?;
    let safety_toggle_ids = payload
        .safety_toggle_ids
        .ok_or_else(|| missing_field_error("safetyToggleIds"))?;
    let unit_system = payload
        .unit_system
        .ok_or_else(|| missing_field_error("unitSystem"))?;

    Ok(ParsedPreferences {
        interest_theme_ids: parse_uuid_list(interest_theme_ids, "interestThemeIds")?,
        safety_toggle_ids: parse_uuid_list(safety_toggle_ids, "safetyToggleIds")?,
        unit_system: parse_unit_system(unit_system)?,
        expected_revision: payload.expected_revision,
    })
}

#[derive(Debug)]
struct ParsedPreferences {
    interest_theme_ids: Vec<Uuid>,
    safety_toggle_ids: Vec<Uuid>,
    unit_system: UnitSystem,
    expected_revision: Option<u32>,
}

/// Fetch the authenticated user's preferences.
#[utoipa::path(
    get,
    path = "/api/v1/users/me/preferences",
    description = "Fetch preferences, creating defaults if none exist.",
    responses(
        (
            status = 200,
            description = "User preferences",
            headers(("Cache-Control" = String, description = "Cache control header")),
            body = UserPreferencesResponse
        ),
        (status = 401, description = "Unauthorised", body = ErrorSchema),
        (status = 500, description = "Internal server error", body = ErrorSchema)
    ),
    tags = ["users"],
    operation_id = "getUserPreferences"
)]
#[get("/users/me/preferences")]
pub async fn get_preferences(
    state: web::Data<HttpState>,
    session: SessionContext,
) -> ApiResult<HttpResponse> {
    let user_id = session.require_user_id()?;
    let preferences = state.preferences_query.fetch_preferences(&user_id).await?;
    Ok(HttpResponse::Ok()
        .insert_header(("Cache-Control", "private, must-revalidate, no-cache"))
        .json(UserPreferencesResponse::from(preferences)))
}

/// Update the authenticated user's preferences.
#[utoipa::path(
    put,
    path = "/api/v1/users/me/preferences",
    request_body = PreferencesRequest,
    responses(
        (status = 200, description = "Updated preferences", body = UserPreferencesResponse),
        (status = 400, description = "Invalid request", body = ErrorSchema),
        (status = 401, description = "Unauthorised", body = ErrorSchema),
        (status = 409, description = "Conflict", body = ErrorSchema),
        (status = 503, description = "Service unavailable", body = ErrorSchema)
    ),
    params(
        ("Idempotency-Key" = Option<String>, Header, description = "UUID for idempotent requests")
    ),
    tags = ["users"],
    operation_id = "updateUserPreferences"
)]
#[put("/users/me/preferences")]
pub async fn update_preferences(
    state: web::Data<HttpState>,
    session: SessionContext,
    request: HttpRequest,
    payload: web::Json<PreferencesRequest>,
) -> ApiResult<web::Json<UserPreferencesResponse>> {
    let user_id = session.require_user_id()?;
    let idempotency_key =
        extract_idempotency_key(request.headers()).map_err(map_idempotency_key_error)?;
    let parsed = parse_preferences_request(payload.into_inner())?;

    let response = state
        .preferences
        .update(UpdatePreferencesRequest {
            user_id,
            interest_theme_ids: parsed.interest_theme_ids,
            safety_toggle_ids: parsed.safety_toggle_ids,
            unit_system: parsed.unit_system,
            expected_revision: parsed.expected_revision,
            idempotency_key,
        })
        .await?;

    Ok(web::Json(UserPreferencesResponse::from(
        response.preferences,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{ErrorCode, UnitSystem, UserId};
    use chrono::Utc;
    use rstest::rstest;

    #[rstest]
    fn parse_preferences_request_rejects_missing_fields() {
        let payload = PreferencesRequest {
            interest_theme_ids: None,
            safety_toggle_ids: Some(Vec::new()),
            unit_system: Some("metric".to_owned()),
            expected_revision: None,
        };

        let err = parse_preferences_request(payload).expect_err("missing interestThemeIds");
        assert_eq!(err.code(), ErrorCode::InvalidRequest);
    }

    #[rstest]
    fn parse_preferences_request_rejects_invalid_unit_system() {
        let payload = PreferencesRequest {
            interest_theme_ids: Some(Vec::new()),
            safety_toggle_ids: Some(Vec::new()),
            unit_system: Some("bad".to_owned()),
            expected_revision: None,
        };

        let err = parse_preferences_request(payload).expect_err("invalid unit system");
        assert_eq!(err.code(), ErrorCode::InvalidRequest);
        let details = err
            .details()
            .and_then(|value| value.as_object())
            .expect("details");
        assert_eq!(
            details.get("field").and_then(|v| v.as_str()),
            Some("unitSystem")
        );
    }

    #[rstest]
    fn user_preferences_response_maps_domain_values() {
        let user_id = UserId::new("11111111-1111-1111-1111-111111111111").expect("user id");
        let preferences = UserPreferences {
            user_id: user_id.clone(),
            interest_theme_ids: vec![Uuid::nil()],
            safety_toggle_ids: vec![Uuid::nil()],
            unit_system: UnitSystem::Metric,
            revision: 2,
            updated_at: Utc::now(),
        };

        let response = UserPreferencesResponse::from(preferences);
        assert_eq!(response.user_id, user_id.to_string());
        assert_eq!(response.revision, 2);
        assert_eq!(response.unit_system, "metric");
    }
}
