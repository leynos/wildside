//! Tests for offline bundle service.

use std::sync::Arc;

use chrono::Utc;
use rstest::rstest;
use serde_json::json;
use uuid::Uuid;

use super::*;
use crate::domain::ports::{
    FixtureIdempotencyRepository, MockIdempotencyRepository, MockOfflineBundleRepository,
    OfflineBundleRepositoryError,
};
use crate::domain::{
    BoundingBox, IdempotencyKey, IdempotencyLookupQuery, IdempotencyLookupResult,
    IdempotencyRecord, MutationType, OfflineBundleKind, OfflineBundleStatus, ZoomRange,
};

fn sample_bundle_payload() -> OfflineBundlePayload {
    OfflineBundlePayload {
        id: Uuid::new_v4(),
        owner_user_id: Some(crate::domain::UserId::random()),
        device_id: "fixture-device".to_owned(),
        kind: OfflineBundleKind::Route,
        route_id: Some(Uuid::new_v4()),
        region_id: None,
        bounds: BoundingBox::new(-3.2, 55.9, -3.0, 56.0).expect("valid bounds"),
        zoom_range: ZoomRange::new(11, 15).expect("valid zoom"),
        estimated_size_bytes: 1_500,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        status: OfflineBundleStatus::Queued,
        progress: 0.0,
    }
}

fn make_service(
    repo: MockOfflineBundleRepository,
) -> OfflineBundleCommandService<MockOfflineBundleRepository, FixtureIdempotencyRepository> {
    OfflineBundleCommandService::new(Arc::new(repo), Arc::new(FixtureIdempotencyRepository))
}

fn make_query_service(
    repo: MockOfflineBundleRepository,
) -> OfflineBundleQueryService<MockOfflineBundleRepository> {
    OfflineBundleQueryService::new(Arc::new(repo))
}

#[tokio::test]
async fn upsert_persists_bundle_without_idempotency_key() {
    let payload = sample_bundle_payload();
    let user_id = payload
        .owner_user_id
        .clone()
        .expect("bundle owner is set for this test");
    let expected_id = payload.id;
    let mut repo = MockOfflineBundleRepository::new();

    repo.expect_find_by_id().times(1).return_once(|_| Ok(None));
    repo.expect_save().times(1).return_once(|_| Ok(()));

    let service = make_service(repo);
    let response = service
        .upsert_bundle(UpsertOfflineBundleRequest {
            user_id,
            bundle: payload,
            idempotency_key: None,
        })
        .await
        .expect("upsert succeeds");

    assert_eq!(response.bundle.id, expected_id);
    assert!(!response.is_replayed);
}

#[tokio::test]
async fn upsert_with_idempotency_stores_bundle_mutation_record() {
    let payload = sample_bundle_payload();
    let user_id = payload
        .owner_user_id
        .clone()
        .expect("bundle owner is set for this test");
    let idempotency_key = IdempotencyKey::random();
    let payload_hash = OfflineBundleCommandService::<
        MockOfflineBundleRepository,
        MockIdempotencyRepository,
    >::hash_payload(&payload)
    .expect("payload hash");

    let mut repo = MockOfflineBundleRepository::new();
    repo.expect_find_by_id().times(1).return_once(|_| Ok(None));
    repo.expect_save().times(1).return_once(|_| Ok(()));

    let lookup_user_id = user_id.clone();
    let lookup_key = idempotency_key.clone();
    let mut idempotency_repo = MockIdempotencyRepository::new();
    idempotency_repo
        .expect_lookup()
        .withf(move |query: &IdempotencyLookupQuery| {
            query.key == lookup_key
                && query.user_id == lookup_user_id
                && query.mutation_type == MutationType::Bundles
                && query.payload_hash == payload_hash
        })
        .times(1)
        .return_once(|_| Ok(IdempotencyLookupResult::NotFound));
    idempotency_repo
        .expect_store()
        .withf(|record: &IdempotencyRecord| record.mutation_type == MutationType::Bundles)
        .times(1)
        .return_once(|_| Ok(()));

    let service = OfflineBundleCommandService::new(Arc::new(repo), Arc::new(idempotency_repo));
    let response = service
        .upsert_bundle(UpsertOfflineBundleRequest {
            user_id,
            bundle: payload,
            idempotency_key: Some(idempotency_key),
        })
        .await
        .expect("upsert succeeds");

    assert!(!response.is_replayed);
}

#[tokio::test]
async fn upsert_rejects_existing_bundle_owned_by_different_user() {
    let payload = sample_bundle_payload();
    let user_id = payload
        .owner_user_id
        .clone()
        .expect("bundle owner is set for this test");
    let mut existing_payload = payload.clone();
    existing_payload.owner_user_id = Some(crate::domain::UserId::random());
    let existing_bundle =
        crate::domain::OfflineBundle::try_from(existing_payload).expect("valid existing bundle");

    let mut repo = MockOfflineBundleRepository::new();
    repo.expect_find_by_id()
        .times(1)
        .return_once(move |_| Ok(Some(existing_bundle)));
    repo.expect_save().times(0);

    let service = make_service(repo);
    let error = service
        .upsert_bundle(UpsertOfflineBundleRequest {
            user_id,
            bundle: payload,
            idempotency_key: None,
        })
        .await
        .expect_err("cross-user upsert must be forbidden");

    assert_eq!(error.code(), crate::domain::ErrorCode::Forbidden);
}

#[tokio::test]
async fn upsert_returns_replayed_response_when_payload_matches() {
    let payload = sample_bundle_payload();
    let user_id = crate::domain::UserId::random();
    let idempotency_key = IdempotencyKey::random();
    let payload_hash = OfflineBundleCommandService::<
        MockOfflineBundleRepository,
        MockIdempotencyRepository,
    >::hash_payload(&payload)
    .expect("payload hash");
    let response_snapshot = serde_json::to_value(UpsertOfflineBundleResponse {
        bundle: payload.clone(),
        is_replayed: false,
    })
    .expect("response snapshot");
    let record = IdempotencyRecord {
        key: idempotency_key.clone(),
        mutation_type: MutationType::Bundles,
        payload_hash: payload_hash.clone(),
        response_snapshot,
        user_id: user_id.clone(),
        created_at: Utc::now(),
    };

    let mut repo = MockOfflineBundleRepository::new();
    repo.expect_save().times(0);

    let mut idempotency_repo = MockIdempotencyRepository::new();
    idempotency_repo
        .expect_lookup()
        .times(1)
        .return_once(move |_| Ok(IdempotencyLookupResult::MatchingPayload(record)));
    idempotency_repo.expect_store().times(0);

    let service = OfflineBundleCommandService::new(Arc::new(repo), Arc::new(idempotency_repo));
    let response = service
        .upsert_bundle(UpsertOfflineBundleRequest {
            user_id,
            bundle: payload,
            idempotency_key: Some(idempotency_key),
        })
        .await
        .expect("replayed response");

    assert!(response.is_replayed);
}

#[tokio::test]
async fn delete_returns_not_found_when_bundle_is_missing() {
    let bundle_id = Uuid::new_v4();
    let mut repo = MockOfflineBundleRepository::new();
    repo.expect_find_by_id().times(1).return_once(|_| Ok(None));
    repo.expect_delete().times(0);

    let service = make_service(repo);
    let error = service
        .delete_bundle(DeleteOfflineBundleRequest {
            user_id: crate::domain::UserId::random(),
            bundle_id,
            idempotency_key: None,
        })
        .await
        .expect_err("not found");

    assert_eq!(error.code(), crate::domain::ErrorCode::NotFound);
}

#[tokio::test]
async fn get_bundle_returns_not_found_for_unknown_id() {
    let bundle_id = Uuid::new_v4();
    let mut repo = MockOfflineBundleRepository::new();
    repo.expect_find_by_id().times(1).return_once(|_| Ok(None));

    let service = make_query_service(repo);
    let error = service
        .get_bundle(GetOfflineBundleRequest { bundle_id })
        .await
        .expect_err("not found");

    assert_eq!(error.code(), crate::domain::ErrorCode::NotFound);
}

#[tokio::test]
async fn list_bundles_maps_connection_error_to_service_unavailable() {
    let mut repo = MockOfflineBundleRepository::new();
    repo.expect_list_for_owner_and_device()
        .times(1)
        .return_once(|_, _| Err(OfflineBundleRepositoryError::connection("pool unavailable")));

    let service = make_query_service(repo);
    let error = service
        .list_bundles(ListOfflineBundlesRequest {
            owner_user_id: None,
            device_id: "fixture-device".to_owned(),
        })
        .await
        .expect_err("service unavailable");

    assert_eq!(error.code(), crate::domain::ErrorCode::ServiceUnavailable);
}

#[tokio::test]
async fn delete_rejects_payload_conflict_for_existing_idempotency_key() {
    let bundle_id = Uuid::new_v4();
    let user_id = crate::domain::UserId::random();
    let idempotency_key = IdempotencyKey::random();
    let payload_hash = OfflineBundleCommandService::<
        MockOfflineBundleRepository,
        MockIdempotencyRepository,
    >::hash_payload(&json!({ "bundleId": bundle_id }))
    .expect("payload hash");
    let conflicting = IdempotencyRecord {
        key: idempotency_key.clone(),
        mutation_type: MutationType::Bundles,
        payload_hash,
        response_snapshot: json!({"bundleId": bundle_id}),
        user_id: user_id.clone(),
        created_at: Utc::now(),
    };

    let mut repo = MockOfflineBundleRepository::new();
    repo.expect_find_by_id().times(0);
    repo.expect_delete().times(0);

    let mut idempotency_repo = MockIdempotencyRepository::new();
    idempotency_repo
        .expect_lookup()
        .times(1)
        .return_once(move |_| Ok(IdempotencyLookupResult::ConflictingPayload(conflicting)));

    let service = OfflineBundleCommandService::new(Arc::new(repo), Arc::new(idempotency_repo));
    let error = service
        .delete_bundle(DeleteOfflineBundleRequest {
            user_id,
            bundle_id,
            idempotency_key: Some(idempotency_key),
        })
        .await
        .expect_err("conflict");

    assert_eq!(error.code(), crate::domain::ErrorCode::Conflict);
}

enum IdempotencyRegressionCase {
    InvalidDeviceId(&'static str),
    UpsertReplay,
    UpsertConflict,
}

#[rstest]
#[case::empty_device_id(IdempotencyRegressionCase::InvalidDeviceId(""))]
#[case::whitespace_device_id(IdempotencyRegressionCase::InvalidDeviceId("  \t\n "))]
#[case::upsert_replay(IdempotencyRegressionCase::UpsertReplay)]
#[case::upsert_conflict(IdempotencyRegressionCase::UpsertConflict)]
#[tokio::test]
async fn idempotency_regression_paths(#[case] case: IdempotencyRegressionCase) {
    match case {
        IdempotencyRegressionCase::InvalidDeviceId(device_id) => {
            idempotency_tests::assert_list_bundles_rejects_invalid_device_id(device_id).await
        }
        IdempotencyRegressionCase::UpsertReplay => {
            idempotency_tests::assert_upsert_replays_response_when_duplicate_key_race_finds_record(
            )
            .await
        }
        IdempotencyRegressionCase::UpsertConflict => idempotency_tests::
            assert_upsert_returns_conflict_when_duplicate_key_race_finds_conflicting_record()
            .await,
    }
}

#[path = "offline_bundle_service_idempotency_tests.rs"]
mod idempotency_tests;
