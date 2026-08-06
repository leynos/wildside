//! Reqwest-backed Overpass source adapter.
//!
//! This adapter owns transport details only: request serialization, timeout and
//! HTTP error mapping, and JSON decoding into domain POIs.

use std::time::Duration;

use async_trait::async_trait;
use reqwest::{Client, StatusCode, Url};

use super::dto::OverpassResponseDto;
use crate::domain::ports::{
    EnrichmentPoi, EnrichmentRequest, EnrichmentResponse, EnrichmentSource, EnrichmentSourceError,
};

const DEFAULT_OVERPASS_QUERY_TIMEOUT_SECONDS: u32 = 180;
const DEFAULT_USER_AGENT: &str = "wildside-backend-overpass-worker/0.1";
const DEFAULT_CONTACT: &str = "ops@wildside.invalid";

/// Outbound identity and query timeout settings for Overpass requests.
pub struct OverpassHttpIdentity {
    /// HTTP user-agent sent to Overpass.
    pub user_agent: String,
    /// Contact header value sent to Overpass.
    pub contact: String,
    /// Timeout directive embedded in Overpass query text.
    pub query_timeout_seconds: u32,
}

impl Default for OverpassHttpIdentity {
    fn default() -> Self {
        Self {
            user_agent: DEFAULT_USER_AGENT.to_owned(),
            contact: DEFAULT_CONTACT.to_owned(),
            query_timeout_seconds: DEFAULT_OVERPASS_QUERY_TIMEOUT_SECONDS,
        }
    }
}

/// Overpass source adapter that performs HTTP POST requests against one endpoint.
pub struct OverpassHttpSource {
    client: Client,
    endpoint: Url,
    user_agent: String,
    contact: String,
    query_timeout_seconds: u32,
}

impl OverpassHttpSource {
    /// Build an adapter using a reqwest client with an explicit request timeout.
    /// ```rust,ignore
    /// let source = OverpassHttpSource::new(endpoint, timeout);
    /// assert!(source.is_ok() || source.is_err());
    /// ```
    /// # Errors
    ///
    /// Returns an error when the reqwest client cannot be constructed.
    pub fn new(endpoint: Url, timeout: Duration) -> Result<Self, reqwest::Error> {
        Self::with_identity(endpoint, timeout, OverpassHttpIdentity::default())
    }

    /// Build an adapter with explicit outbound identity and query timeout.
    /// ```rust,ignore
    /// let source = OverpassHttpSource::with_identity(endpoint, timeout, identity);
    /// assert!(source.is_ok() || source.is_err());
    /// ```
    /// # Errors
    ///
    /// Returns an error when the reqwest client cannot be constructed.
    pub fn with_identity(
        endpoint: Url,
        timeout: Duration,
        identity: OverpassHttpIdentity,
    ) -> Result<Self, reqwest::Error> {
        let client = Client::builder().timeout(timeout).build()?;
        Ok(Self {
            client,
            endpoint,
            user_agent: identity.user_agent,
            contact: identity.contact,
            query_timeout_seconds: identity.query_timeout_seconds.max(1),
        })
    }
}

#[async_trait]
impl EnrichmentSource for OverpassHttpSource {
    async fn fetch_pois(
        &self,
        request: &EnrichmentRequest,
    ) -> Result<EnrichmentResponse, EnrichmentSourceError> {
        let query = build_overpass_query(request, self.query_timeout_seconds)?;
        let response = self
            .client
            .post(self.endpoint.clone())
            .header(reqwest::header::USER_AGENT, self.user_agent.as_str())
            .header("Contact", self.contact.as_str())
            .header(reqwest::header::ACCEPT, "application/json")
            .form(&[("data", query)])
            .send()
            .await
            .map_err(map_transport_error)?;

        let status = response.status();
        let body = response.bytes().await.map_err(map_transport_error)?;
        if !status.is_success() {
            return Err(map_status_error(status, body.as_ref()));
        }

        let transfer_bytes = body.len() as u64;
        let pois = parse_pois(body.as_ref())?;
        Ok(EnrichmentResponse {
            pois,
            transfer_bytes,
            source_url: self.endpoint.to_string(),
        })
    }
}

fn parse_pois(body: &[u8]) -> Result<Vec<EnrichmentPoi>, EnrichmentSourceError> {
    let decoded: OverpassResponseDto = serde_json::from_slice(body).map_err(|error| {
        EnrichmentSourceError::decode(format!("invalid Overpass JSON payload: {error}"))
    })?;
    decoded
        .into_domain_pois()
        .map_err(EnrichmentSourceError::decode)
}

fn build_overpass_query(
    request: &EnrichmentRequest,
    query_timeout_seconds: u32,
) -> Result<String, EnrichmentSourceError> {
    // `BoundingBox` has already enforced WGS84 validity, so the adapter only
    // reorders the coordinates into Overpass's south,west,north,east form.
    let [min_lng, min_lat, max_lng, max_lat] = request.bounding_box.as_array();
    let bbox = format!("({min_lat},{min_lng},{max_lat},{max_lng})");

    let selectors = if request.tags.is_empty() {
        vec![String::new()]
    } else {
        request
            .tags
            .iter()
            .map(|tag| build_tag_selector(tag))
            .collect::<Result<Vec<_>, _>>()?
    };

    let mut lines = Vec::with_capacity(selectors.len() * 3);
    for selector in selectors {
        for element_type in ["node", "way", "relation"] {
            lines.push(format!("  {element_type}{selector}{bbox};"));
        }
    }

    Ok(format!(
        "[out:json][timeout:{query_timeout_seconds}];\n(\n{query_lines}\n);\nout center tags;",
        query_lines = lines.join("\n")
    ))
}

fn build_tag_selector(tag: &str) -> Result<String, EnrichmentSourceError> {
    let trimmed = tag.trim();
    if trimmed.is_empty() {
        return Err(EnrichmentSourceError::invalid_request(
            "tags must not include blank values",
        ));
    }

    let (key, maybe_value) = match trimmed.split_once('=') {
        Some((key, value)) => (key.trim(), Some(value.trim())),
        None => (trimmed, None),
    };
    if key.is_empty() {
        return Err(EnrichmentSourceError::invalid_request(
            "tags must provide a non-empty key",
        ));
    }

    let escaped_key = escape_quoted(key);
    match maybe_value {
        Some("") => Err(EnrichmentSourceError::invalid_request(
            "tags must not include empty values",
        )),
        Some(value) => Ok(format!("[\"{escaped_key}\"=\"{}\"]", escape_quoted(value))),
        None => Ok(format!("[\"{escaped_key}\"]")),
    }
}

fn escape_quoted(raw: &str) -> String {
    raw.replace('\\', r"\\").replace('"', "\\\"")
}

fn map_transport_error(error: reqwest::Error) -> EnrichmentSourceError {
    if error.is_timeout() {
        EnrichmentSourceError::timeout(error.to_string())
    } else {
        EnrichmentSourceError::transport(error.to_string())
    }
}

fn map_status_error(status: StatusCode, body: &[u8]) -> EnrichmentSourceError {
    let body_preview = body_preview(body);
    let message = if body_preview.is_empty() {
        format!("status {}", status.as_u16())
    } else {
        format!("status {}: {}", status.as_u16(), body_preview)
    };

    match status {
        StatusCode::TOO_MANY_REQUESTS => EnrichmentSourceError::rate_limited(message),
        StatusCode::REQUEST_TIMEOUT | StatusCode::GATEWAY_TIMEOUT => {
            EnrichmentSourceError::timeout(message)
        }
        _ if status.is_client_error() => EnrichmentSourceError::invalid_request(message),
        _ => EnrichmentSourceError::transport(message),
    }
}

fn body_preview(body: &[u8]) -> String {
    const PREVIEW_CHAR_LIMIT: usize = 160;

    let compact = String::from_utf8_lossy(body)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let preview = compact.chars().take(PREVIEW_CHAR_LIMIT).collect::<String>();
    if compact.chars().count() > PREVIEW_CHAR_LIMIT {
        format!("{preview}...")
    } else {
        preview
    }
}

#[cfg(test)]
mod tests {
    //! Regression coverage for non-network Overpass mapping helpers.

    use super::*;
    use crate::domain::BoundingBox;
    use rstest::rstest;
    use uuid::Uuid;

    fn request(tags: Vec<&str>) -> EnrichmentRequest {
        let bounding_box = BoundingBox::new(-3.30, 55.90, -3.10, 56.00)
            .expect("fixture bounding box should be valid");
        EnrichmentRequest {
            job_id: Uuid::new_v4(),
            bounding_box,
            tags: tags.into_iter().map(str::to_owned).collect(),
        }
    }

    #[test]
    fn builds_query_with_bbox_reordered_for_overpass() {
        let query = build_overpass_query(&request(vec!["amenity", "name=coffee \"bar\""]), 180)
            .expect("query should build");

        assert!(
            query.contains("node[\"amenity\"](55.9,-3.3,56,-3.1);"),
            "query should include bbox in south,west,north,east order"
        );
        assert!(
            query.starts_with("[out:json][timeout:180];"),
            "query should include configured timeout"
        );
        assert!(
            query.contains("way[\"name\"=\"coffee \\\"bar\\\"\"](55.9,-3.3,56,-3.1);"),
            "query should escape quoted values in tag selectors"
        );
    }

    #[rstest]
    #[case::rate_limited(StatusCode::TOO_MANY_REQUESTS, "RateLimited")]
    #[case::request_timeout(StatusCode::REQUEST_TIMEOUT, "Timeout")]
    #[case::gateway_timeout(StatusCode::GATEWAY_TIMEOUT, "Timeout")]
    #[case::bad_request(StatusCode::BAD_REQUEST, "InvalidRequest")]
    #[case::server_error(StatusCode::INTERNAL_SERVER_ERROR, "Transport")]
    fn maps_http_statuses_to_expected_domain_errors(
        #[case] status: StatusCode,
        #[case] expected: &str,
    ) {
        let error = map_status_error(status, b"{\"remark\":\"backend unavailable\"}");
        match expected {
            "RateLimited" => {
                assert!(
                    matches!(error, EnrichmentSourceError::RateLimited { .. }),
                    "429 should map to RateLimited",
                );
            }
            "Timeout" => {
                assert!(
                    matches!(error, EnrichmentSourceError::Timeout { .. }),
                    "timeout statuses should map to Timeout",
                );
            }
            "InvalidRequest" => {
                assert!(
                    matches!(error, EnrichmentSourceError::InvalidRequest { .. }),
                    "client statuses should map to InvalidRequest",
                );
            }
            "Transport" => {
                assert!(
                    matches!(error, EnrichmentSourceError::Transport { .. }),
                    "other statuses should map to Transport",
                );
            }
            _ => panic!("unsupported test expectation: {expected}"),
        }
    }

    #[test]
    fn parses_overpass_json_into_domain_pois() {
        let body = r#"{
            "elements": [
                {
                    "type": "node",
                    "id": 101,
                    "lat": 55.91,
                    "lon": -3.21,
                    "tags": { "amenity": "cafe" }
                },
                {
                    "type": "way",
                    "id": 102,
                    "center": { "lat": 55.92, "lon": -3.22 },
                    "tags": { "name": "The Meadows" }
                }
            ]
        }"#;

        let pois = parse_pois(body.as_bytes()).expect("JSON should decode");
        assert_eq!(pois.len(), 2, "two POIs should be decoded");
        assert_eq!(pois[0].element_type, "node");
        assert_eq!(pois[0].longitude, -3.21);
        assert_eq!(pois[1].element_type, "way");
        assert_eq!(pois[1].latitude, 55.92);
    }

    #[test]
    fn rejects_elements_without_coordinates() {
        let body = r#"{
            "elements": [
                { "type": "way", "id": 201, "tags": { "name": "missing-centre" } }
            ]
        }"#;

        let error = parse_pois(body.as_bytes()).expect_err("decode should fail");
        assert!(
            matches!(error, EnrichmentSourceError::Decode { .. }),
            "missing coordinates should map to Decode errors",
        );
    }

    #[rstest]
    #[case::longitude_out_of_range(-181.0, 55.90, -3.10, 56.00)]
    #[case::latitude_out_of_range(-3.30, -91.0, -3.10, 56.00)]
    fn bbox_outside_wgs84_ranges_cannot_reach_the_adapter(
        #[case] min_lng: f64,
        #[case] min_lat: f64,
        #[case] max_lng: f64,
        #[case] max_lat: f64,
    ) {
        // `BoundingBox` owns WGS84 validity, so an out-of-range box is
        // unrepresentable in `EnrichmentRequest` and never reaches query
        // building. Range-by-range coverage lives with `BoundingBox` itself.
        assert!(
            BoundingBox::new(min_lng, min_lat, max_lng, max_lat).is_err(),
            "out-of-range coordinates must be rejected before an \
             EnrichmentRequest can be constructed",
        );
    }
}
