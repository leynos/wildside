//! Additional idempotency and device validation tests for offline bundle service.

#![cfg(test)]

use std::{error::Error as StdError, io, sync::Arc};

use chrono::{DateTime, Utc};
use mockable::DefaultClock;
use rstest::rstest;
use serde_json::json;

use super::*;
use crate::domain::ports::{
    IdempotencyRepositoryError, MockIdempotencyRepository, MockOfflineBundleRepository,
};
use crate::domain::{IdempotencyLookupResult, IdempotencyRecord, MutationType};

type TestResult<T = ()> = Result<T, Box<dyn StdError>>;

fn fixture_timestamp() -> TestResult<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339("2026-01-02T03:04:05Z")?.with_timezone(&Utc))
}

pub(super) async fn assert_list_bundles_rejects_invalid_device_id(device_id: &str) -> TestResult {
    let mut repo = MockOfflineBundleRepository::new();
    repo.expect_list_for_owner_and_device().times(0);

    let service = make_query_service(repo);
    let error = service
        .list_bundles(ListOfflineBundlesRequest {
            owner_user_id: Some(crate::domain::UserId::random()),
            device_id: device_id.to_owned(),
        })
        .await
        .expect_err("invalid device id should be rejected");

    assert_eq!(error.code(), crate::domain::ErrorCode::InvalidRequest);
    Ok(())
}

#[rstest]
#[case("")]
#[case("  \t\n ")]
#[tokio::test]
async fn list_bundles_rejects_invalid_device_id(#[case] device_id: &str) -> TestResult {
    assert_list_bundles_rejects_invalid_device_id(device_id).await
}

pub(super) async fn assert_upsert_replays_response_when_duplicate_key_race_finds_record()
-> TestResult {
    let payload = sample_bundle_payload()?;
    let user_id = payload
        .owner_user_id
        .clone()
        .ok_or_else(|| io::Error::other("bundle owner is set for this test"))?;
    let idempotency_key = IdempotencyKey::random();
    let payload_hash = OfflineBundleCommandService::<
        MockOfflineBundleRepository,
        MockIdempotencyRepository,
    >::hash_payload(&payload)?;
    let response_snapshot = serde_json::to_value(UpsertOfflineBundleResponse {
        bundle: payload.clone(),
        is_replayed: false,
    })?;
    let record = IdempotencyRecord {
        key: idempotency_key.clone(),
        mutation_type: MutationType::Bundles,
        payload_hash: payload_hash.clone(),
        response_snapshot,
        user_id: user_id.clone(),
        created_at: fixture_timestamp()?,
    };

    let mut repo = MockOfflineBundleRepository::new();
    repo.expect_find_by_id().times(0);
    repo.expect_save().times(0);

    let mut idempotency_repo = MockIdempotencyRepository::new();
    idempotency_repo
        .expect_store_in_progress()
        .times(1)
        .return_once(|_| {
            Err(IdempotencyRepositoryError::DuplicateKey {
                message: "race".to_owned(),
            })
        });

    idempotency_repo
        .expect_lookup()
        .times(1)
        .return_once(move |_| Ok(IdempotencyLookupResult::MatchingPayload(record)));

    let service = OfflineBundleCommandService::new(
        Arc::new(repo),
        Arc::new(idempotency_repo),
        Arc::new(DefaultClock),
    );
    let response = service
        .upsert_bundle(UpsertOfflineBundleRequest {
            user_id,
            bundle: payload,
            idempotency_key: Some(idempotency_key),
        })
        .await?;

    assert!(response.is_replayed);
    Ok(())
}

pub(super) async fn assert_upsert_returns_conflict_when_duplicate_key_race_finds_conflicting_record()
-> TestResult {
    let payload = sample_bundle_payload()?;
    let user_id = payload
        .owner_user_id
        .clone()
        .ok_or_else(|| io::Error::other("bundle owner is set for this test"))?;
    let idempotency_key = IdempotencyKey::random();
    let payload_hash = OfflineBundleCommandService::<
        MockOfflineBundleRepository,
        MockIdempotencyRepository,
    >::hash_payload(&payload)?;
    let conflicting_record = IdempotencyRecord {
        key: idempotency_key.clone(),
        mutation_type: MutationType::Bundles,
        payload_hash,
        response_snapshot: json!({"bundleId": payload.id}),
        user_id: user_id.clone(),
        created_at: fixture_timestamp()?,
    };

    let mut repo = MockOfflineBundleRepository::new();
    repo.expect_find_by_id().times(0);
    repo.expect_save().times(0);

    let mut idempotency_repo = MockIdempotencyRepository::new();
    idempotency_repo
        .expect_store_in_progress()
        .times(1)
        .return_once(|_| {
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

    let service = OfflineBundleCommandService::new(
        Arc::new(repo),
        Arc::new(idempotency_repo),
        Arc::new(DefaultClock),
    );
    let error = service
        .upsert_bundle(UpsertOfflineBundleRequest {
            user_id,
            bundle: payload,
            idempotency_key: Some(idempotency_key),
        })
        .await
        .expect_err("conflict");

    assert_eq!(error.code(), crate::domain::ErrorCode::Conflict);
    Ok(())
}
