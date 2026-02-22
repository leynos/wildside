//! Additional idempotency and device validation tests for offline bundle service.

use std::sync::Arc;

use chrono::Utc;
use serde_json::json;

use super::*;
use crate::domain::ports::{
    IdempotencyRepositoryError, MockIdempotencyRepository, MockOfflineBundleRepository,
};
use crate::domain::{
    IdempotencyLookupQuery, IdempotencyLookupResult, IdempotencyRecord, MutationType,
};

#[tokio::test]
async fn list_bundles_rejects_empty_device_id() {
    let mut repo = MockOfflineBundleRepository::new();
    repo.expect_list_for_owner_and_device().times(0);

    let service = make_query_service(repo);
    let error = service
        .list_bundles(ListOfflineBundlesRequest {
            owner_user_id: Some(crate::domain::UserId::random()),
            device_id: String::new(),
        })
        .await
        .expect_err("empty device id should be rejected");

    assert_eq!(error.code(), crate::domain::ErrorCode::InvalidRequest);
}

#[tokio::test]
async fn list_bundles_rejects_whitespace_device_id() {
    let mut repo = MockOfflineBundleRepository::new();
    repo.expect_list_for_owner_and_device().times(0);

    let service = make_query_service(repo);
    let error = service
        .list_bundles(ListOfflineBundlesRequest {
            owner_user_id: Some(crate::domain::UserId::random()),
            device_id: "  \t\n ".to_owned(),
        })
        .await
        .expect_err("whitespace device id should be rejected");

    assert_eq!(error.code(), crate::domain::ErrorCode::InvalidRequest);
}

#[tokio::test]
async fn upsert_replays_response_when_duplicate_key_race_finds_record() {
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
    let response_snapshot = serde_json::to_value(UpsertOfflineBundleResponse {
        bundle: payload.clone(),
        replayed: false,
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
    repo.expect_save().times(1).return_once(|_| Ok(()));

    let mut idempotency_repo = MockIdempotencyRepository::new();
    let idempotency_key_for_first_lookup = idempotency_key.clone();
    let user_id_for_first_lookup = user_id.clone();
    idempotency_repo
        .expect_lookup()
        .withf(move |query: &IdempotencyLookupQuery| {
            query.key == idempotency_key_for_first_lookup
                && query.user_id == user_id_for_first_lookup
                && query.payload_hash == payload_hash
                && query.mutation_type == MutationType::Bundles
        })
        .times(1)
        .return_once(|_| Ok(IdempotencyLookupResult::NotFound));

    idempotency_repo.expect_store().times(1).return_once(|_| {
        Err(IdempotencyRepositoryError::DuplicateKey {
            message: "race".to_owned(),
        })
    });

    idempotency_repo
        .expect_lookup()
        .times(1)
        .return_once(move |_| Ok(IdempotencyLookupResult::MatchingPayload(record)));

    let service = OfflineBundleCommandService::new(Arc::new(repo), Arc::new(idempotency_repo));
    let response = service
        .upsert_bundle(UpsertOfflineBundleRequest {
            user_id,
            bundle: payload,
            idempotency_key: Some(idempotency_key),
        })
        .await
        .expect("replayed response");

    assert!(response.replayed);
}

#[tokio::test]
async fn upsert_returns_conflict_when_duplicate_key_race_finds_conflicting_record() {
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
    let conflicting_record = IdempotencyRecord {
        key: idempotency_key.clone(),
        mutation_type: MutationType::Bundles,
        payload_hash,
        response_snapshot: json!({"bundleId": payload.id}),
        user_id: user_id.clone(),
        created_at: Utc::now(),
    };

    let mut repo = MockOfflineBundleRepository::new();
    repo.expect_save().times(1).return_once(|_| Ok(()));

    let mut idempotency_repo = MockIdempotencyRepository::new();
    idempotency_repo
        .expect_lookup()
        .times(1)
        .return_once(|_| Ok(IdempotencyLookupResult::NotFound));
    idempotency_repo.expect_store().times(1).return_once(|_| {
        Err(IdempotencyRepositoryError::DuplicateKey {
            message: "race".to_owned(),
        })
    });
    idempotency_repo
        .expect_lookup()
        .times(1)
        .return_once(move |_| {
            Ok(IdempotencyLookupResult::ConflictingPayload(
                conflicting_record,
            ))
        });

    let service = OfflineBundleCommandService::new(Arc::new(repo), Arc::new(idempotency_repo));
    let error = service
        .upsert_bundle(UpsertOfflineBundleRequest {
            user_id,
            bundle: payload,
            idempotency_key: Some(idempotency_key),
        })
        .await
        .expect_err("conflict");

    assert_eq!(error.code(), crate::domain::ErrorCode::Conflict);
}
