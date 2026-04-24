//! Request-validation tests for users API payload parsing.

use super::super::{
    INTEREST_THEME_IDS_MAX, InterestsRequest, map_interests_request_error, parse_interest_theme_ids,
};
use super::TestResult;
use crate::domain::ErrorCode;
use rstest::rstest;
use serde_json::Value;
use std::io;

fn as_object(value: &Value) -> io::Result<&serde_json::Map<String, Value>> {
    value
        .as_object()
        .ok_or_else(|| io::Error::other("expected JSON object"))
}

#[rstest]
#[case("", "empty_interest_theme_id", "interest theme id must not be empty")]
#[case(
    "not-a-uuid",
    "invalid_interest_theme_id",
    "interest theme id must be a valid UUID"
)]
fn interests_request_validation_rejects_invalid_ids(
    #[case] value: &str,
    #[case] expected_code: &str,
    #[case] expected_message: &str,
) -> TestResult {
    let payload = InterestsRequest {
        interest_theme_ids: vec![value.to_owned()],
        expected_revision: None,
    };

    let err = parse_interest_theme_ids(payload).expect_err("invalid interest theme id");
    let api_error = map_interests_request_error(err);

    assert_eq!(api_error.code(), ErrorCode::InvalidRequest);
    assert_eq!(api_error.message(), expected_message);
    let details = as_object(
        api_error
            .details()
            .ok_or_else(|| io::Error::other("expected error details payload to be present"))?,
    )?;
    assert_eq!(
        details.get("code").and_then(Value::as_str),
        Some(expected_code)
    );
    assert_eq!(
        details.get("field").and_then(Value::as_str),
        Some("interestThemeIds")
    );
    assert_eq!(details.get("index").and_then(Value::as_u64), Some(0));
    Ok(())
}

#[test]
fn interests_request_validation_rejects_too_many_ids() -> TestResult {
    let payload = InterestsRequest {
        interest_theme_ids: vec![
            "3fa85f64-5717-4562-b3fc-2c963f66afa6".to_owned();
            INTEREST_THEME_IDS_MAX + 1
        ],
        expected_revision: None,
    };

    let err = parse_interest_theme_ids(payload).expect_err("too many ids");
    let api_error = map_interests_request_error(err);

    assert_eq!(api_error.code(), ErrorCode::InvalidRequest);
    assert_eq!(
        api_error.message(),
        "interest theme ids must contain at most 100 items"
    );
    let details = as_object(
        api_error
            .details()
            .ok_or_else(|| io::Error::other("expected error details payload to be present"))?,
    )?;
    assert_eq!(
        details.get("code").and_then(Value::as_str),
        Some("too_many_interest_theme_ids")
    );
    assert_eq!(
        details.get("field").and_then(Value::as_str),
        Some("interestThemeIds")
    );
    assert_eq!(
        details.get("max").and_then(Value::as_u64),
        Some(INTEREST_THEME_IDS_MAX as u64)
    );
    assert_eq!(
        details.get("count").and_then(Value::as_u64),
        Some((INTEREST_THEME_IDS_MAX + 1) as u64)
    );
    Ok(())
}

#[test]
fn interests_request_validation_accepts_valid_ids() -> TestResult {
    let payload = InterestsRequest {
        interest_theme_ids: vec!["3fa85f64-5717-4562-b3fc-2c963f66afa6".to_owned()],
        expected_revision: Some(7),
    };

    let parsed = parse_interest_theme_ids(payload).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("failed to parse interest theme ids: {error:?}"),
        )
    })?;
    assert_eq!(parsed.expected_revision, Some(7));
    assert_eq!(parsed.interest_theme_ids.len(), 1);
    assert_eq!(
        parsed.interest_theme_ids[0].as_ref(),
        "3fa85f64-5717-4562-b3fc-2c963f66afa6"
    );
    Ok(())
}
