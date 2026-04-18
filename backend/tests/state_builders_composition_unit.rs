//! Unit tests proving fixture-mode selection for config-selected HTTP state
//! ports.
//!
//! These tests verify the fixture half of the `build_http_state` composition
//! invariant: when `ServerConfig.db_pool` is `None`, the ports selected from
//! `ServerConfig` resolve to fixture adapters. `route_submission` is supplied
//! directly by the caller to `build_http_state`, so this module only checks
//! that the supplied `Arc` is forwarded unchanged rather than derived from
//! `ServerConfig`. DB-mode composition is covered by
//! `startup_mode_composition_bdd`.

use std::sync::Arc;

use actix_web::web;
use backend::test_support::server::{ServerConfig, build_http_state};
use backend::{
    domain::ports::{
        CreateWalkSessionRequest, DeleteNoteRequest, DeleteOfflineBundleRequest,
        FixtureRouteSubmissionService, GetOfflineBundleRequest, GetWalkSessionRequest,
        ListEnrichmentProvenanceRequest, ListOfflineBundlesRequest,
        ListWalkCompletionSummariesRequest, RouteSubmissionRequest, RouteSubmissionService,
        RouteSubmissionStatus, UpdatePreferencesRequest, UpdateProgressRequest,
        UpdateUserInterestsRequest, UpsertNoteRequest, UpsertOfflineBundleRequest,
    },
    domain::{ErrorCode, IdempotencyKey, InterestThemeId, UnitSystem, UserId},
    inbound::http::state::HttpState,
};
use rstest::rstest;
use uuid::Uuid;

#[path = "state_builders_composition_unit/support.rs"]
mod support;

use support::{fixture_config, sample_bundle_payload, sample_walk_session};

async fn login_and_get_user_id(state: &web::Data<HttpState>) -> UserId {
    use backend::domain::LoginCredentials;

    let fixture_creds =
        LoginCredentials::try_from_parts("admin", "password").expect("valid credentials");
    let login_result = state.login.authenticate(&fixture_creds).await;
    assert!(
        login_result.is_ok(),
        "fixture login should accept admin/password; got: {login_result:?}"
    );
    let user_id = login_result.expect("user id");
    assert_eq!(
        user_id.as_ref(),
        "123e4567-e89b-12d3-a456-426614174000",
        "fixture login returns fixed user ID"
    );
    user_id
}

async fn users_and_profile_ok(state: &web::Data<HttpState>, user_id: &UserId) {
    let users_result = state.users.list_users(user_id).await;
    assert!(
        users_result.is_ok(),
        "fixture users query should succeed; got: {users_result:?}"
    );

    let profile_result = state.profile.fetch_profile(user_id).await;
    assert!(
        profile_result.is_ok(),
        "fixture profile query should succeed; got: {profile_result:?}"
    );
}

async fn interests_flow(state: &web::Data<HttpState>, user_id: &UserId) {
    let interests_result = state
        .interests
        .set_interests(UpdateUserInterestsRequest {
            user_id: user_id.clone(),
            interest_theme_ids: vec![InterestThemeId::from_uuid(Uuid::new_v4())],
            expected_revision: Some(1),
        })
        .await;
    assert!(
        interests_result.is_ok(),
        "fixture interests command should succeed; got: {interests_result:?}"
    );
    assert_eq!(interests_result.expect("interests").revision(), 2);
}

async fn preferences_write_and_read_ok(state: &web::Data<HttpState>, user_id: &UserId) {
    let preferences_result = state
        .preferences
        .update(UpdatePreferencesRequest {
            user_id: user_id.clone(),
            interest_theme_ids: vec![Uuid::new_v4()],
            safety_toggle_ids: vec![Uuid::new_v4()],
            unit_system: UnitSystem::Imperial,
            expected_revision: Some(1),
            idempotency_key: Some(IdempotencyKey::random()),
        })
        .await;
    assert!(
        preferences_result.is_ok(),
        "fixture preferences command should succeed; got: {preferences_result:?}"
    );
    let preferences = preferences_result.expect("preferences response");
    assert!(!preferences.replayed);
    assert_eq!(preferences.preferences.revision, 2);
    assert_eq!(preferences.preferences.unit_system, UnitSystem::Imperial);

    let prefs_result = state.preferences_query.fetch_preferences(user_id).await;
    assert!(
        prefs_result.is_ok(),
        "fixture preferences query should succeed; got: {prefs_result:?}"
    );
}

async fn route_annotations_flow(state: &web::Data<HttpState>, user_id: &UserId, route_id: Uuid) {
    let annotations_result = state
        .route_annotations_query
        .fetch_annotations(route_id, user_id)
        .await;
    assert!(
        annotations_result.is_ok(),
        "fixture route annotations query should succeed; got: {annotations_result:?}"
    );
    assert_eq!(annotations_result.expect("annotations").route_id, route_id);

    let upsert_note_result = state
        .route_annotations
        .upsert_note(UpsertNoteRequest {
            note_id: Uuid::new_v4(),
            route_id,
            poi_id: None,
            user_id: user_id.clone(),
            body: "Fixture note".to_owned(),
            expected_revision: Some(2),
            idempotency_key: None,
        })
        .await;
    assert!(
        upsert_note_result.is_ok(),
        "fixture route note upsert should succeed; got: {upsert_note_result:?}"
    );
    assert_eq!(upsert_note_result.expect("note response").note.revision, 3);

    let delete_note_result = state
        .route_annotations
        .delete_note(DeleteNoteRequest {
            note_id: Uuid::new_v4(),
            user_id: user_id.clone(),
            idempotency_key: Some(IdempotencyKey::random()),
        })
        .await;
    assert!(
        delete_note_result.is_ok(),
        "fixture route note delete should succeed; got: {delete_note_result:?}"
    );
    assert!(!delete_note_result.expect("delete note response").deleted);

    let update_progress_result = state
        .route_annotations
        .update_progress(UpdateProgressRequest {
            route_id,
            user_id: user_id.clone(),
            visited_stop_ids: vec![Uuid::new_v4()],
            expected_revision: Some(4),
            idempotency_key: None,
        })
        .await;
    assert!(
        update_progress_result.is_ok(),
        "fixture route progress update should succeed; got: {update_progress_result:?}"
    );
    assert_eq!(
        update_progress_result
            .expect("progress response")
            .progress
            .revision,
        5
    );
}

async fn route_submission_flow(state: &web::Data<HttpState>, user_id: &UserId) {
    let route_submission_result = state
        .route_submission
        .submit(RouteSubmissionRequest {
            idempotency_key: Some(IdempotencyKey::random()),
            user_id: user_id.clone(),
            payload: serde_json::json!({"origin": "A", "destination": "B"}),
        })
        .await;
    assert!(
        route_submission_result.is_ok(),
        "fixture route submission should succeed; got: {route_submission_result:?}"
    );
    assert_eq!(
        route_submission_result
            .expect("route submission response")
            .status,
        RouteSubmissionStatus::Accepted
    );
}

async fn catalogue_and_descriptors_ok(state: &web::Data<HttpState>) {
    let catalogue_result = state.catalogue.explore_snapshot().await;
    assert!(
        catalogue_result.is_ok(),
        "fixture catalogue should succeed; got: {catalogue_result:?}"
    );

    let descriptors_result = state.descriptors.descriptor_snapshot().await;
    assert!(
        descriptors_result.is_ok(),
        "fixture descriptors should succeed; got: {descriptors_result:?}"
    );
    assert!(
        descriptors_result
            .expect("descriptors")
            .interest_themes
            .is_empty()
    );
}

async fn upsert_offline_bundle(
    state: &web::Data<HttpState>,
    user_id: &UserId,
    bundle: &backend::domain::ports::OfflineBundlePayload,
) {
    let offline_upsert_result = state
        .offline_bundles
        .upsert_bundle(UpsertOfflineBundleRequest {
            user_id: user_id.clone(),
            bundle: bundle.clone(),
            idempotency_key: None,
        })
        .await;
    assert!(
        offline_upsert_result.is_ok(),
        "fixture offline bundle upsert should succeed; got: {offline_upsert_result:?}"
    );
    assert_eq!(
        offline_upsert_result
            .expect("offline upsert response")
            .bundle
            .id,
        bundle.id
    );
}

async fn delete_offline_bundle(state: &web::Data<HttpState>, user_id: &UserId, bundle_id: Uuid) {
    let offline_delete_result = state
        .offline_bundles
        .delete_bundle(DeleteOfflineBundleRequest {
            user_id: user_id.clone(),
            bundle_id,
            idempotency_key: Some(IdempotencyKey::random()),
        })
        .await;
    assert!(
        offline_delete_result.is_ok(),
        "fixture offline bundle delete should succeed; got: {offline_delete_result:?}"
    );
    assert_eq!(
        offline_delete_result
            .expect("offline delete response")
            .bundle_id,
        bundle_id
    );
}

async fn list_and_get_offline_bundles(
    state: &web::Data<HttpState>,
    user_id: &UserId,
    device_id: &str,
    bundle_id: Uuid,
) {
    let offline_list_result = state
        .offline_bundles_query
        .list_bundles(ListOfflineBundlesRequest {
            owner_user_id: Some(user_id.clone()),
            device_id: device_id.to_owned(),
        })
        .await;
    assert!(
        offline_list_result.is_ok(),
        "fixture offline bundle list should succeed; got: {offline_list_result:?}"
    );
    assert!(
        offline_list_result
            .expect("offline list response")
            .bundles
            .is_empty()
    );

    let offline_get_result = state
        .offline_bundles_query
        .get_bundle(GetOfflineBundleRequest { bundle_id })
        .await;
    assert!(
        offline_get_result.is_err(),
        "fixture offline bundle get should be not found; got: {offline_get_result:?}"
    );
    assert_eq!(
        offline_get_result.expect_err("offline get error").code(),
        ErrorCode::NotFound,
    );
}

async fn offline_bundles_flow(state: &web::Data<HttpState>, user_id: &UserId, route_id: Uuid) {
    let bundle = sample_bundle_payload(user_id, route_id);
    upsert_offline_bundle(state, user_id, &bundle).await;
    delete_offline_bundle(state, user_id, bundle.id).await;
    list_and_get_offline_bundles(state, user_id, "fixture-device", bundle.id).await;
}

async fn enrichment_provenance_ok(state: &web::Data<HttpState>) {
    let provenance_result = state
        .enrichment_provenance
        .list_recent(&ListEnrichmentProvenanceRequest {
            limit: 10,
            before: None,
        })
        .await;
    assert!(
        provenance_result.is_ok(),
        "fixture enrichment provenance should succeed; got: {provenance_result:?}"
    );
}

async fn walk_sessions_flow(state: &web::Data<HttpState>, user_id: &UserId, route_id: Uuid) {
    let walk_session = sample_walk_session(user_id, route_id);

    let walk_create_result = state
        .walk_sessions
        .create_session(CreateWalkSessionRequest {
            session: walk_session.clone(),
        })
        .await;
    assert!(
        walk_create_result.is_ok(),
        "fixture walk session create should succeed; got: {walk_create_result:?}"
    );
    assert_eq!(
        walk_create_result
            .expect("walk session response")
            .session_id,
        walk_session.id
    );

    let walk_get_result = state
        .walk_sessions_query
        .get_session(GetWalkSessionRequest {
            session_id: walk_session.id,
        })
        .await;
    assert!(
        walk_get_result.is_err(),
        "fixture walk session get should be not found; got: {walk_get_result:?}"
    );
    assert_eq!(
        walk_get_result.expect_err("walk session get error").code(),
        ErrorCode::NotFound,
    );

    let walk_list_result = state
        .walk_sessions_query
        .list_completion_summaries(ListWalkCompletionSummariesRequest {
            user_id: user_id.clone(),
        })
        .await;
    assert!(
        walk_list_result.is_ok(),
        "fixture walk completion summary list should succeed; got: {walk_list_result:?}"
    );
    assert!(
        walk_list_result
            .expect("walk summaries")
            .summaries
            .is_empty()
    );
}

/// Test that fixture mode exhibits fixture behaviour across every port.
#[rstest]
#[tokio::test]
async fn fixture_mode_wires_fixture_adapters(fixture_config: ServerConfig) {
    let route_submission: Arc<dyn RouteSubmissionService> = Arc::new(FixtureRouteSubmissionService);
    let state = build_http_state(&fixture_config, route_submission.clone());
    assert!(Arc::ptr_eq(
        &state.get_ref().route_submission,
        &route_submission
    ));

    let user_id = login_and_get_user_id(&state).await;
    users_and_profile_ok(&state, &user_id).await;
    interests_flow(&state, &user_id).await;
    preferences_write_and_read_ok(&state, &user_id).await;

    let route_id = Uuid::new_v4();
    route_annotations_flow(&state, &user_id, route_id).await;
    route_submission_flow(&state, &user_id).await;
    catalogue_and_descriptors_ok(&state).await;
    offline_bundles_flow(&state, &user_id, route_id).await;
    enrichment_provenance_ok(&state).await;
    walk_sessions_flow(&state, &user_id, route_id).await;
}
