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
#[path = "http_source/tests.rs"]
mod tests;
