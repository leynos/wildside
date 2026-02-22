//! Shared validation helpers for inbound HTTP adapters.

use chrono::{DateTime, Utc};
use serde_json::json;
use uuid::Uuid;

use crate::domain::Error;

pub(crate) fn missing_field_error(field: &str) -> Error {
    Error::invalid_request(format!("missing required field: {field}")).with_details(json!({
        "field": field,
        "code": "missing_field",
    }))
}

pub(crate) fn invalid_uuid_error(field: &str, value: &str) -> Error {
    Error::invalid_request(format!("{field} must be a valid UUID")).with_details(json!({
        "field": field,
        "value": value,
        "code": "invalid_uuid",
    }))
}

pub(crate) fn invalid_uuid_index_error(field: &str, index: usize, value: &str) -> Error {
    Error::invalid_request(format!("{field} must contain valid UUIDs")).with_details(json!({
        "field": field,
        "index": index,
        "value": value,
        "code": "invalid_uuid",
    }))
}

pub(crate) fn parse_uuid(value: String, field: &str) -> Result<Uuid, Error> {
    Uuid::parse_str(&value).map_err(|_| invalid_uuid_error(field, &value))
}

pub(crate) fn parse_uuid_list(values: Vec<String>, field: &str) -> Result<Vec<Uuid>, Error> {
    values
        .into_iter()
        .enumerate()
        .map(|(index, value)| {
            Uuid::parse_str(&value).map_err(|_| invalid_uuid_index_error(field, index, &value))
        })
        .collect()
}

pub(crate) fn invalid_timestamp_error(field: &str, value: &str) -> Error {
    Error::invalid_request(format!("{field} must be an RFC 3339 timestamp")).with_details(json!({
        "field": field,
        "value": value,
        "code": "invalid_timestamp",
    }))
}

pub(crate) fn parse_rfc3339_timestamp(value: String, field: &str) -> Result<DateTime<Utc>, Error> {
    DateTime::parse_from_rfc3339(&value)
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .map_err(|_| invalid_timestamp_error(field, &value))
}

pub(crate) fn parse_optional_rfc3339_timestamp(
    value: Option<String>,
    field: &str,
) -> Result<Option<DateTime<Utc>>, Error> {
    value
        .map(|raw| parse_rfc3339_timestamp(raw, field))
        .transpose()
}
