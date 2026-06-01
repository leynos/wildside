//! Cursor error tests for the users list endpoint.

use actix_web::test as actix_test;
use insta::assert_json_snapshot;
use serde_json::Value;

use super::{TestResult, get_details_object, login_and_get_cookie, test_app};

#[actix_web::test]
async fn list_users_rejects_invalid_cursor() -> TestResult {
    assert_users_cursor_error(
        "/api/v1/users?cursor=not-a-cursor",
        "cursor is invalid",
        "invalid_cursor",
        "list_users_invalid_cursor_error_envelope",
    )
    .await
}

#[actix_web::test]
async fn list_users_rejects_unsupported_cursor_direction() -> TestResult {
    let unsupported_direction_cursor = "eyJrZXkiOnsiY3JlYXRlZF9hdCI6IjIwMjYtMDMtMjJUMTA6MzA6MDBaIiwiaWQiOiIxMTExMTExMS0xMTExLTExMTEtMTExMS0xMTExMTExMTExMTEifSwiZGlyIjoiU2lkZXdheXMifQ";
    let path = format!("/api/v1/users?cursor={unsupported_direction_cursor}");

    assert_users_cursor_error(
        &path,
        "cursor direction is unsupported",
        "unsupported_direction",
        "list_users_unsupported_direction_error_envelope",
    )
    .await
}

async fn assert_users_cursor_error(
    path: &str,
    expected_message: &str,
    expected_detail_code: &str,
    snapshot_name: &str,
) -> TestResult {
    let app = actix_test::init_service(test_app()).await;
    let cookie = login_and_get_cookie(&app).await?;

    let users_req = actix_test::TestRequest::get()
        .uri(path)
        .cookie(cookie)
        .to_request();
    let users_res = actix_test::call_service(&app, users_req).await;

    assert_eq!(users_res.status(), actix_web::http::StatusCode::BAD_REQUEST);
    let body = actix_test::read_body(users_res).await;
    let value: Value = serde_json::from_slice(&body)?;
    assert_eq!(
        value.get("message").and_then(Value::as_str),
        Some(expected_message)
    );
    let details = get_details_object(&value)?;
    assert_eq!(details.get("field").and_then(Value::as_str), Some("cursor"));
    assert_eq!(
        details.get("code").and_then(Value::as_str),
        Some(expected_detail_code)
    );
    assert_json_snapshot!(snapshot_name, value);
    Ok(())
}
