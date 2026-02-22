//! Shared validation helpers for inbound HTTP adapters.

use chrono::{DateTime, Utc};
use serde_json::json;
use uuid::Uuid;

use crate::domain::Error;

/// Error codes for validation failures.
const ERR_MISSING_FIELD: &str = "missing_field";
const ERR_INVALID_UUID: &str = "invalid_uuid";
const ERR_INVALID_TIMESTAMP: &str = "invalid_timestamp";

/// Builder for validation errors with field context.
struct ValidationError {
    field: String,
    message: String,
}

impl ValidationError {
    fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }

    fn with_code(self, code: &'static str) -> Error {
        Error::invalid_request(self.message).with_details(json!({
            "field": self.field,
            "code": code,
        }))
    }

    fn with_value(self, code: &'static str, value: impl Into<String>) -> Error {
        Error::invalid_request(self.message).with_details(json!({
            "field": self.field,
            "value": value.into(),
            "code": code,
        }))
    }

    fn with_index(self, code: &'static str, index: usize, value: impl Into<String>) -> Error {
        Error::invalid_request(self.message).with_details(json!({
            "field": self.field,
            "index": index,
            "value": value.into(),
            "code": code,
        }))
    }
}

pub(crate) fn missing_field_error(field: &str) -> Error {
    ValidationError::new(field, format!("missing required field: {field}"))
        .with_code(ERR_MISSING_FIELD)
}

pub(crate) fn invalid_uuid_error(field: &str, value: &str) -> Error {
    ValidationError::new(field, format!("{field} must be a valid UUID"))
        .with_value(ERR_INVALID_UUID, value)
}

pub(crate) fn invalid_uuid_index_error(field: &str, index: usize, value: &str) -> Error {
    ValidationError::new(field, format!("{field} must contain valid UUIDs")).with_index(
        ERR_INVALID_UUID,
        index,
        value,
    )
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
    ValidationError::new(field, format!("{field} must be an RFC 3339 timestamp"))
        .with_value(ERR_INVALID_TIMESTAMP, value)
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
