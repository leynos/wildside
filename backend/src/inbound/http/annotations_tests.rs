//! Tests for route annotations HTTP handlers.

use super::*;
use crate::domain::ErrorCode;
use rstest::rstest;

#[rstest]
fn parse_note_request_rejects_missing_fields() {
    let payload = NoteRequest {
        note_id: None,
        poi_id: None,
        body: Some("note".to_owned()),
        expected_revision: None,
    };

    let err = parse_note_request(payload).expect_err("missing noteId");
    assert_eq!(err.code(), ErrorCode::InvalidRequest);
}

#[rstest]
fn parse_note_request_rejects_invalid_uuid() {
    let payload = NoteRequest {
        note_id: Some("bad".to_owned()),
        poi_id: None,
        body: Some("note".to_owned()),
        expected_revision: None,
    };

    let err = parse_note_request(payload).expect_err("invalid note id");
    assert_eq!(err.code(), ErrorCode::InvalidRequest);
    let details = err
        .details()
        .and_then(|value| value.as_object())
        .expect("details");
    assert_eq!(
        details.get("field").and_then(|v| v.as_str()),
        Some("noteId")
    );
}

#[rstest]
fn parse_route_id_rejects_invalid_uuid() {
    let err = parse_route_id(RoutePath {
        route_id: "bad".to_owned(),
    })
    .expect_err("invalid route id");
    assert_eq!(err.code(), ErrorCode::InvalidRequest);
}

#[rstest]
fn parse_progress_request_rejects_missing_list() {
    let payload = ProgressRequest {
        visited_stop_ids: None,
        expected_revision: None,
    };

    let err = parse_progress_request(payload).expect_err("missing visitedStopIds");
    assert_eq!(err.code(), ErrorCode::InvalidRequest);
}

#[rstest]
fn parse_progress_request_rejects_invalid_uuid() {
    let payload = ProgressRequest {
        visited_stop_ids: Some(vec!["bad".to_owned()]),
        expected_revision: None,
    };

    let err = parse_progress_request(payload).expect_err("invalid stop id");
    assert_eq!(err.code(), ErrorCode::InvalidRequest);
    let details = err
        .details()
        .and_then(|value| value.as_object())
        .expect("details");
    assert_eq!(
        details.get("field").and_then(|v| v.as_str()),
        Some("visitedStopIds")
    );
}
