//! Shared validation helpers for inbound HTTP adapters.

use chrono::{DateTime, Utc};
use serde_json::json;
use uuid::Uuid;

use crate::domain::Error;

/// Validation error codes for HTTP request failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ErrorCode {
    MissingField,
    InvalidUuid,
    InvalidTimestamp,
}

impl ErrorCode {
    fn as_str(self) -> &'static str {
        match self {
            ErrorCode::MissingField => "missing_field",
            ErrorCode::InvalidUuid => "invalid_uuid",
            ErrorCode::InvalidTimestamp => "invalid_timestamp",
        }
    }
}

/// Newtype wrapper for HTTP field names to provide type safety.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FieldName(&'static str);

impl FieldName {
    pub(crate) const fn new(name: &'static str) -> Self {
        Self(name)
    }

    fn as_str(&self) -> &str {
        self.0
    }
}

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

    fn with_code(self, code: ErrorCode) -> Error {
        Error::invalid_request(self.message).with_details(json!({
            "field": self.field,
            "code": code.as_str(),
        }))
    }

    fn with_value(self, code: ErrorCode, value: impl Into<String>) -> Error {
        Error::invalid_request(self.message).with_details(json!({
            "field": self.field,
            "value": value.into(),
            "code": code.as_str(),
        }))
    }

    fn with_index(self, code: ErrorCode, index: usize, value: impl Into<String>) -> Error {
        Error::invalid_request(self.message).with_details(json!({
            "field": self.field,
            "index": index,
            "value": value.into(),
            "code": code.as_str(),
        }))
    }
}

pub(crate) fn missing_field_error(field: FieldName) -> Error {
    let field = field.as_str();
    ValidationError::new(field, format!("missing required field: {field}"))
        .with_code(ErrorCode::MissingField)
}

pub(crate) fn invalid_uuid_error(field: FieldName, value: &str) -> Error {
    let field = field.as_str();
    ValidationError::new(field, format!("{field} must be a valid UUID"))
        .with_value(ErrorCode::InvalidUuid, value)
}

pub(crate) fn invalid_uuid_index_error(field: FieldName, index: usize, value: &str) -> Error {
    let field = field.as_str();
    ValidationError::new(field, format!("{field} must contain valid UUIDs")).with_index(
        ErrorCode::InvalidUuid,
        index,
        value,
    )
}

pub(crate) fn parse_uuid(value: String, field: FieldName) -> Result<Uuid, Error> {
    Uuid::parse_str(&value).map_err(|_| invalid_uuid_error(field, &value))
}

pub(crate) fn parse_uuid_list(values: Vec<String>, field: FieldName) -> Result<Vec<Uuid>, Error> {
    values
        .into_iter()
        .enumerate()
        .map(|(index, value)| {
            Uuid::parse_str(&value).map_err(|_| invalid_uuid_index_error(field, index, &value))
        })
        .collect()
}

pub(crate) fn invalid_timestamp_error(field: FieldName, value: &str) -> Error {
    let field = field.as_str();
    ValidationError::new(field, format!("{field} must be an RFC 3339 timestamp"))
        .with_value(ErrorCode::InvalidTimestamp, value)
}

pub(crate) fn parse_rfc3339_timestamp(
    value: String,
    field: FieldName,
) -> Result<DateTime<Utc>, Error> {
    DateTime::parse_from_rfc3339(&value)
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .map_err(|_| invalid_timestamp_error(field, &value))
}

pub(crate) fn parse_optional_rfc3339_timestamp(
    value: Option<String>,
    field: FieldName,
) -> Result<Option<DateTime<Utc>>, Error> {
    value
        .map(|raw| parse_rfc3339_timestamp(raw, field))
        .transpose()
}
