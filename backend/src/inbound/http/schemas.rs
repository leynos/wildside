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
    /// The request conflicts with existing state (e.g., idempotency key reuse with different payload).
    #[schema(rename = "conflict")]
    Conflict,
    /// The service is temporarily unavailable (e.g., idempotency store unavailable).
    #[schema(rename = "service_unavailable")]
    ServiceUnavailable,
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
    #[schema(rename = "traceId", example = "01HZY8B2W6X5Y7Z9ABCD1234")]
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
    ///
    /// Matches the domain `User` invariant of being a UUID.
    #[schema(
        value_type = String,
        format = "uuid",
        example = "3fa85f64-5717-4562-b3fc-2c963f66afa6"
    )]
    id: String,
    /// Display name shown to other users.
    ///
    /// Schema constraints: 3–32 characters, alphanumeric plus spaces and
    /// underscores. The domain layer additionally validates that the value
    /// is non-empty when trimmed.
    #[schema(
        rename = "displayName",
        value_type = String,
        min_length = 3,
        max_length = 32,
        pattern = "^[A-Za-z0-9_ ]+$",
        example = "Ada Lovelace"
    )]
    display_name: String,
}

/// OpenAPI schema for [`crate::domain::InterestThemeId`].
///
/// Interest theme identifiers are UUIDs serialized as strings.
#[derive(ToSchema)]
#[schema(
    as = crate::domain::InterestThemeId,
    value_type = String,
    format = "uuid",
    example = "3fa85f64-5717-4562-b3fc-2c963f66afa6"
)]
pub struct InterestThemeIdSchema(pub String);

/// OpenAPI schema for [`crate::domain::UserInterests`].
///
/// User interest selections with theme identifiers.
#[derive(ToSchema)]
#[schema(as = crate::domain::UserInterests)]
#[expect(
    dead_code,
    reason = "Used only for OpenAPI schema generation via utoipa"
)]
pub struct UserInterestsSchema {
    /// Stable user identifier.
    #[schema(
        rename = "userId",
        value_type = String,
        format = "uuid",
        example = "11111111-1111-1111-1111-111111111111"
    )]
    user_id: String,
    /// Selected interest theme identifiers.
    #[schema(
        rename = "interestThemeIds",
        value_type = Vec<InterestThemeIdSchema>,
        max_items = 100
    )]
    interest_theme_ids: Vec<String>,
    /// Shared aggregate revision after the interests update.
    revision: u32,
}

#[cfg(test)]
mod tests {
    //! Regression coverage for this module.
    use super::*;
    use std::error::Error as StdError;
    use utoipa::PartialSchema;

    type TestResult<T = ()> = Result<T, Box<dyn StdError>>;

    fn schema_to_json<T: PartialSchema>() -> serde_json::Result<String> {
        serde_json::to_string(&T::schema())
    }

    fn assert_schema_contains<T: PartialSchema + ToSchema>(
        expected_name: &str,
        expected_fields: &[&str],
    ) -> TestResult {
        let schema_json = schema_to_json::<T>()?;
        let name = T::name();
        // utoipa replaces :: with . in schema names
        assert_eq!(name, expected_name);
        for field in expected_fields {
            assert!(
                schema_json.contains(field),
                "schema should contain {field} field"
            );
        }
        Ok(())
    }

    #[test]
    fn error_code_schema_has_expected_name() -> TestResult {
        assert_schema_contains::<ErrorCodeSchema>("crate.domain.ErrorCode", &["invalid_request"])
    }

    #[test]
    fn error_schema_has_expected_name() -> TestResult {
        assert_schema_contains::<ErrorSchema>("crate.domain.Error", &["message", "traceId"])
    }

    #[test]
    fn user_schema_has_expected_name() -> TestResult {
        assert_schema_contains::<UserSchema>("crate.domain.User", &["displayName"])
    }

    #[test]
    fn interest_theme_id_schema_has_expected_name() -> TestResult {
        assert_schema_contains::<InterestThemeIdSchema>("crate.domain.InterestThemeId", &["uuid"])
    }

    #[test]
    fn user_interests_schema_has_expected_name() -> TestResult {
        assert_schema_contains::<UserInterestsSchema>(
            "crate.domain.UserInterests",
            &["interestThemeIds", "revision"],
        )
    }

    #[test]
    fn error_code_schema_variants_match_domain() -> TestResult {
        // Verify the schema contains all expected error code variants
        let schema_json = schema_to_json::<ErrorCodeSchema>()?;
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
        assert!(schema_json.contains("conflict"), "missing conflict");
        assert!(
            schema_json.contains("service_unavailable"),
            "missing service_unavailable"
        );
        Ok(())
    }

    /// Verify domain ErrorCode serialization matches schema renames.
    ///
    /// Uses pattern matching for compile-time exhaustiveness: if a new domain
    /// variant is added, this test will fail to compile until updated.
    #[test]
    fn error_code_schema_matches_domain_serialization() -> TestResult {
        use crate::domain::ErrorCode;

        /// Map domain variant to expected serialized string.
        ///
        /// Pattern matching ensures compile-time exhaustiveness: adding a new
        /// `ErrorCode` variant without updating this function causes a build error.
        fn expected_serialization(code: ErrorCode) -> &'static str {
            match code {
                ErrorCode::InvalidRequest => "invalid_request",
                ErrorCode::Unauthorized => "unauthorized",
                ErrorCode::Forbidden => "forbidden",
                ErrorCode::NotFound => "not_found",
                ErrorCode::Conflict => "conflict",
                ErrorCode::ServiceUnavailable => "service_unavailable",
                ErrorCode::InternalError => "internal_error",
            }
        }

        let variants = [
            ErrorCode::InvalidRequest,
            ErrorCode::Unauthorized,
            ErrorCode::Forbidden,
            ErrorCode::NotFound,
            ErrorCode::Conflict,
            ErrorCode::ServiceUnavailable,
            ErrorCode::InternalError,
        ];

        for code in variants {
            let serialized = serde_json::to_string(&code)?;
            let expected = expected_serialization(code);
            assert_eq!(
                serialized,
                format!("\"{expected}\""),
                "domain ErrorCode::{code:?} should serialize to {expected}"
            );
        }
        Ok(())
    }
}
