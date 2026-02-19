//! Catalogue and descriptor read endpoints.
//!
//! ```text
//! GET /api/v1/catalogue/explore
//! GET /api/v1/catalogue/descriptors
//! ```

use actix_web::{HttpResponse, get, web};
use serde::Serialize;
use utoipa::ToSchema;

use crate::domain::Error;
use crate::domain::ports::{DescriptorSnapshot, ExploreCatalogueSnapshot};
use crate::inbound::http::ApiResult;
use crate::inbound::http::cache_control::private_no_cache_header;
use crate::inbound::http::schemas::ErrorSchema;
use crate::inbound::http::session::SessionContext;
use crate::inbound::http::state::HttpState;

/// Convert a serializable value to `serde_json::Value`, mapping errors to
/// `domain::Error::internal`.
///
/// # Examples
///
/// ```ignore
/// let value = vec!["coastal", "scenic"];
/// let json = to_json_value(value).expect("serializable value");
/// assert_eq!(json, serde_json::json!(["coastal", "scenic"]));
/// ```
fn to_json_value<T: Serialize>(value: T) -> Result<serde_json::Value, Error> {
    serde_json::to_value(value).map_err(|err| Error::internal(err.to_string()))
}

/// Response payload for the explore catalogue snapshot.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExploreCatalogueResponse {
    /// ISO 8601 timestamp when the snapshot was assembled.
    #[schema(example = "2026-01-15T12:00:00Z")]
    pub generated_at: String,
    /// Route categories for browse navigation.
    #[schema(value_type = Vec<serde_json::Value>)]
    pub categories: serde_json::Value,
    /// Route summaries for listing cards.
    #[schema(value_type = Vec<serde_json::Value>)]
    pub routes: serde_json::Value,
    /// Thematic groupings.
    #[schema(value_type = Vec<serde_json::Value>)]
    pub themes: serde_json::Value,
    /// Curated route collections.
    #[schema(value_type = Vec<serde_json::Value>)]
    pub collections: serde_json::Value,
    /// Trending route highlights.
    #[schema(value_type = Vec<serde_json::Value>)]
    pub trending: serde_json::Value,
    /// Current community pick, if any.
    #[schema(value_type = Option<serde_json::Value>)]
    pub community_pick: Option<serde_json::Value>,
}

impl TryFrom<ExploreCatalogueSnapshot> for ExploreCatalogueResponse {
    type Error = Error;

    fn try_from(snapshot: ExploreCatalogueSnapshot) -> Result<Self, Self::Error> {
        Ok(Self {
            generated_at: snapshot.generated_at.to_rfc3339(),
            categories: to_json_value(snapshot.categories)?,
            routes: to_json_value(snapshot.routes)?,
            themes: to_json_value(snapshot.themes)?,
            collections: to_json_value(snapshot.collections)?,
            trending: to_json_value(snapshot.trending)?,
            community_pick: snapshot.community_pick.map(to_json_value).transpose()?,
        })
    }
}

/// Response payload for the descriptor registries snapshot.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DescriptorsResponse {
    /// ISO 8601 timestamp when the snapshot was assembled.
    #[schema(example = "2026-01-15T12:00:00Z")]
    pub generated_at: String,
    /// Tag descriptors.
    #[schema(value_type = Vec<serde_json::Value>)]
    pub tags: serde_json::Value,
    /// Badge descriptors.
    #[schema(value_type = Vec<serde_json::Value>)]
    pub badges: serde_json::Value,
    /// Safety toggle descriptors.
    #[schema(value_type = Vec<serde_json::Value>)]
    pub safety_toggles: serde_json::Value,
    /// Safety preset descriptors.
    #[schema(value_type = Vec<serde_json::Value>)]
    pub safety_presets: serde_json::Value,
    /// Interest theme descriptors.
    #[schema(value_type = Vec<serde_json::Value>)]
    pub interest_themes: serde_json::Value,
}

impl TryFrom<DescriptorSnapshot> for DescriptorsResponse {
    type Error = Error;

    fn try_from(snapshot: DescriptorSnapshot) -> Result<Self, Self::Error> {
        Ok(Self {
            generated_at: snapshot.generated_at.to_rfc3339(),
            tags: to_json_value(snapshot.tags)?,
            badges: to_json_value(snapshot.badges)?,
            safety_toggles: to_json_value(snapshot.safety_toggles)?,
            safety_presets: to_json_value(snapshot.safety_presets)?,
            interest_themes: to_json_value(snapshot.interest_themes)?,
        })
    }
}

/// Fetch the explore catalogue snapshot.
#[utoipa::path(
    get,
    path = "/api/v1/catalogue/explore",
    description = "Return the explore catalogue snapshot for the Progressive Web App (PWA) landing page. Example request: GET /api/v1/catalogue/explore",
    responses(
        (
            status = 200,
            description = "Catalogue snapshot",
            headers(("Cache-Control" = String, description = "Cache control header")),
            body = ExploreCatalogueResponse
        ),
        (status = 401, description = "Unauthorised", body = ErrorSchema),
        (status = 503, description = "Service unavailable", body = ErrorSchema),
        (status = 500, description = "Internal server error", body = ErrorSchema)
    ),
    tags = ["catalogue"],
    operation_id = "getExploreCatalogue",
    security(("SessionCookie" = []))
)]
#[get("/catalogue/explore")]
pub async fn get_explore_catalogue(
    state: web::Data<HttpState>,
    session: SessionContext,
) -> ApiResult<HttpResponse> {
    let _user_id = session.require_user_id()?;
    let snapshot = state.catalogue.explore_snapshot().await?;
    let response = ExploreCatalogueResponse::try_from(snapshot)?;
    Ok(HttpResponse::Ok()
        .insert_header(private_no_cache_header())
        .json(response))
}

/// Fetch the descriptor registries snapshot.
#[utoipa::path(
    get,
    path = "/api/v1/catalogue/descriptors",
    description = "Return all descriptor registries for the Progressive Web App (PWA). Example request: GET /api/v1/catalogue/descriptors",
    responses(
        (
            status = 200,
            description = "Descriptor snapshot",
            headers(("Cache-Control" = String, description = "Cache control header")),
            body = DescriptorsResponse
        ),
        (status = 401, description = "Unauthorised", body = ErrorSchema),
        (status = 503, description = "Service unavailable", body = ErrorSchema),
        (status = 500, description = "Internal server error", body = ErrorSchema)
    ),
    tags = ["catalogue"],
    operation_id = "getDescriptors",
    security(("SessionCookie" = []))
)]
#[get("/catalogue/descriptors")]
pub async fn get_descriptors(
    state: web::Data<HttpState>,
    session: SessionContext,
) -> ApiResult<HttpResponse> {
    let _user_id = session.require_user_id()?;
    let snapshot = state.descriptors.descriptor_snapshot().await?;
    let response = DescriptorsResponse::try_from(snapshot)?;
    Ok(HttpResponse::Ok()
        .insert_header(private_no_cache_header())
        .json(response))
}

#[cfg(test)]
mod tests {
    //! Regression coverage for this module.
    use super::*;
    use crate::domain::ports::empty_catalogue_and_descriptor_snapshots;
    use crate::domain::ports::{DescriptorSnapshot, ExploreCatalogueSnapshot};
    use chrono::DateTime;
    use rstest::{fixture, rstest};

    #[fixture]
    fn catalogue_snapshot() -> ExploreCatalogueSnapshot {
        empty_catalogue_and_descriptor_snapshots().0
    }

    #[fixture]
    fn descriptor_snapshot() -> DescriptorSnapshot {
        empty_catalogue_and_descriptor_snapshots().1
    }

    #[rstest]
    fn explore_response_maps_generated_at_to_rfc3339(catalogue_snapshot: ExploreCatalogueSnapshot) {
        let response = ExploreCatalogueResponse::try_from(catalogue_snapshot)
            .expect("ExploreCatalogueSnapshot should convert to ExploreCatalogueResponse");
        DateTime::parse_from_rfc3339(response.generated_at.as_str())
            .expect("generated_at should be a valid RFC 3339 timestamp");
    }

    #[rstest]
    fn explore_response_maps_empty_catalogue(catalogue_snapshot: ExploreCatalogueSnapshot) {
        let response = ExploreCatalogueResponse::try_from(catalogue_snapshot)
            .expect("ExploreCatalogueSnapshot should convert to ExploreCatalogueResponse");
        assert_eq!(response.categories, serde_json::json!([]));
        assert_eq!(response.routes, serde_json::json!([]));
        assert_eq!(response.themes, serde_json::json!([]));
        assert_eq!(response.collections, serde_json::json!([]));
        assert_eq!(response.trending, serde_json::json!([]));
        assert!(response.community_pick.is_none());
    }

    #[rstest]
    fn descriptors_response_maps_generated_at_to_rfc3339(descriptor_snapshot: DescriptorSnapshot) {
        let response = DescriptorsResponse::try_from(descriptor_snapshot)
            .expect("DescriptorSnapshot should convert to DescriptorsResponse");
        DateTime::parse_from_rfc3339(response.generated_at.as_str())
            .expect("generated_at should be a valid RFC 3339 timestamp");
    }

    #[rstest]
    fn descriptors_response_maps_empty_descriptors(descriptor_snapshot: DescriptorSnapshot) {
        let response = DescriptorsResponse::try_from(descriptor_snapshot)
            .expect("DescriptorSnapshot should convert to DescriptorsResponse");
        assert_eq!(response.tags, serde_json::json!([]));
        assert_eq!(response.badges, serde_json::json!([]));
        assert_eq!(response.safety_toggles, serde_json::json!([]));
        assert_eq!(response.safety_presets, serde_json::json!([]));
        assert_eq!(response.interest_themes, serde_json::json!([]));
    }
}
