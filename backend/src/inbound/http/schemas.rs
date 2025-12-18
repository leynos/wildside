//! OpenAPI schema definitions for domain types.
//!
//! Domain types remain framework-agnostic by not deriving `ToSchema`. This
//! module provides the schema definitions required for OpenAPI documentation
//! using utoipa's external schema registration.
//!
//! The schema wrappers mirror the structure of their corresponding domain
//! types but live in the inbound adapter layer where framework concerns belong.

use utoipa::ToSchema;

/// OpenAPI schema for [`crate::domain::ErrorCode`].
///
/// Stable machine-readable error codes returned in API error responses.
#[derive(ToSchema)]
#[schema(as = crate::domain::ErrorCode)]
pub enum ErrorCodeSchema {
    /// The request is malformed or fails validation.
    #[schema(rename = "invalid_request")]
    InvalidRequest,
    /// Authentication failed or is missing.
    #[schema(rename = "unauthorized")]
    Unauthorized,
    /// Authenticated but not permitted to perform this action.
    #[schema(rename = "forbidden")]
    Forbidden,
    /// The requested resource does not exist.
    #[schema(rename = "not_found")]
    NotFound,
    /// An unexpected error occurred on the server.
    #[schema(rename = "internal_error")]
    InternalError,
}

/// OpenAPI schema for [`crate::domain::Error`].
///
/// API error response payload with machine-readable code and human-readable
/// message.
#[derive(ToSchema)]
#[schema(as = crate::domain::Error)]
#[expect(
    dead_code,
    reason = "Used only for OpenAPI schema generation via utoipa"
)]
pub struct ErrorSchema {
    /// Stable machine-readable error code.
    #[schema(example = "invalid_request")]
    code: ErrorCodeSchema,
    /// Human-readable message returned to clients.
    #[schema(example = "Something went wrong")]
    message: String,
    /// Correlation identifier for tracing this error across systems.
    #[schema(example = "01HZY8B2W6X5Y7Z9ABCD1234")]
    trace_id: Option<String>,
    /// Supplementary error details for clients.
    details: Option<serde_json::Value>,
}

/// OpenAPI schema for [`crate::domain::User`].
///
/// Application user with stable identifier and display name.
#[derive(ToSchema)]
#[schema(as = crate::domain::User)]
#[expect(
    dead_code,
    reason = "Used only for OpenAPI schema generation via utoipa"
)]
pub struct UserSchema {
    /// Stable user identifier.
    #[schema(value_type = String, example = "3fa85f64-5717-4562-b3fc-2c963f66afa6")]
    id: String,
    /// Display name shown to other users.
    #[schema(value_type = String, example = "Ada Lovelace")]
    display_name: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use utoipa::PartialSchema;

    fn schema_to_json<T: PartialSchema>() -> String {
        serde_json::to_string(&T::schema()).expect("schema serialises to JSON")
    }

    #[test]
    fn error_code_schema_has_expected_name() {
        let schema_json = schema_to_json::<ErrorCodeSchema>();
        let name = ErrorCodeSchema::name();
        // utoipa replaces :: with . in schema names
        assert_eq!(name, "crate.domain.ErrorCode");
        assert!(
            schema_json.contains("invalid_request"),
            "schema should contain error code variants"
        );
    }

    #[test]
    fn error_schema_has_expected_name() {
        let schema_json = schema_to_json::<ErrorSchema>();
        let name = ErrorSchema::name();
        // utoipa replaces :: with . in schema names
        assert_eq!(name, "crate.domain.Error");
        assert!(
            schema_json.contains("message"),
            "schema should contain message field"
        );
        assert!(
            schema_json.contains("trace_id"),
            "schema should contain trace_id field"
        );
    }

    #[test]
    fn user_schema_has_expected_name() {
        let schema_json = schema_to_json::<UserSchema>();
        let name = UserSchema::name();
        // utoipa replaces :: with . in schema names
        assert_eq!(name, "crate.domain.User");
        assert!(
            schema_json.contains("display_name"),
            "schema should contain display_name field"
        );
    }

    #[test]
    fn error_code_schema_variants_match_domain() {
        // Verify the schema contains all expected error code variants
        let schema_json = schema_to_json::<ErrorCodeSchema>();
        assert!(
            schema_json.contains("invalid_request"),
            "missing invalid_request"
        );
        assert!(schema_json.contains("unauthorized"), "missing unauthorized");
        assert!(schema_json.contains("forbidden"), "missing forbidden");
        assert!(schema_json.contains("not_found"), "missing not_found");
        assert!(
            schema_json.contains("internal_error"),
            "missing internal_error"
        );
    }
}
