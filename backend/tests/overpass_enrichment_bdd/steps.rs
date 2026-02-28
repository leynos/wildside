//! Step definitions and scenario bindings for Overpass enrichment BDD tests.

use super::*;
use backend::domain::ErrorCode;
use rstest_bdd_macros::{given, then, when};
use std::time::Duration;
use tokio::time::timeout;

#[given("a Diesel-backed Overpass enrichment worker with successful source data")]
fn a_diesel_backed_overpass_enrichment_worker_with_successful_source_data(
    world: &OverpassEnrichmentWorld,
) {
    let source_data = vec![Ok(world.make_response(2, 768))];
    world.setup_with_config_and_data(|_config| {}, source_data);
}

#[given("a Diesel-backed Overpass enrichment worker with exhausted request quota")]
fn a_diesel_backed_overpass_enrichment_worker_with_exhausted_request_quota(
    world: &OverpassEnrichmentWorld,
) {
    let source_data = vec![Ok(world.make_response(1, 128))];
    world.setup_with_config_and_data(
        |config| {
            config.max_daily_requests = 0;
        },
        source_data,
    );
}

#[given("a Diesel-backed Overpass enrichment worker with failing source responses")]
fn a_diesel_backed_overpass_enrichment_worker_with_failing_source_responses(
    world: &OverpassEnrichmentWorld,
) {
    let source_data = vec![
        Err(OverpassEnrichmentSourceError::transport("boom-1")),
        Err(OverpassEnrichmentSourceError::transport("boom-2")),
    ];
    world.setup_with_config_and_data(
        |config| {
            config.max_attempts = 1;
            config.circuit_failure_threshold = 2;
            config.circuit_open_cooldown = Duration::from_secs(300);
        },
        source_data,
    );
}

#[given("a Diesel-backed Overpass enrichment worker with recovery source responses")]
fn a_diesel_backed_overpass_enrichment_worker_with_recovery_source_responses(
    world: &OverpassEnrichmentWorld,
) {
    let source_data = vec![
        Err(OverpassEnrichmentSourceError::transport("boom-once")),
        Ok(world.make_response(1, 256)),
    ];
    world.setup_with_config_and_data(
        |config| {
            config.max_attempts = 1;
            config.circuit_failure_threshold = 1;
            config.circuit_open_cooldown = Duration::from_secs(60);
        },
        source_data,
    );
}

#[given("a Diesel-backed Overpass enrichment worker with retry-exhaustion source responses")]
fn a_diesel_backed_overpass_enrichment_worker_with_retry_exhaustion_source_responses(
    world: &OverpassEnrichmentWorld,
) {
    let source_data = vec![
        Err(OverpassEnrichmentSourceError::timeout("timeout-1")),
        Err(OverpassEnrichmentSourceError::transport("timeout-2")),
    ];
    world.setup_with_config_and_data(
        |config| {
            config.max_attempts = 2;
            config.circuit_failure_threshold = 3;
        },
        source_data,
    );
}

#[given("a Diesel-backed Overpass enrichment worker with semaphore-blocking source responses")]
fn a_diesel_backed_overpass_enrichment_worker_with_semaphore_blocking_source_responses(
    world: &OverpassEnrichmentWorld,
) {
    let source_data = vec![
        Ok(world.make_response(1, 128)),
        Ok(world.make_response(1, 128)),
    ];
    world.setup_with_config_and_data(
        |config| {
            config.max_attempts = 1;
            config.max_concurrent_calls = 1;
        },
        source_data,
    );

    if world.skip_if_needed() {
        return;
    }
    let source = world.source.get().expect("source should be set");
    let (entered_rx, release_notify) = source.enable_blocking();
    world.entered_rx.set(Arc::new(AsyncMutex::new(entered_rx)));
    world.release_notify.set(release_notify);
}

#[when("an enrichment job runs for launch-a bounds")]
fn an_enrichment_job_runs_for_launch_a_bounds(world: &OverpassEnrichmentWorld) {
    world.run_job();
}

#[when("two enrichment jobs run for launch-a bounds")]
fn two_enrichment_jobs_run_for_launch_a_bounds(world: &OverpassEnrichmentWorld) {
    world.run_job();
    world.run_job();
}

#[when("a third enrichment job runs for launch-a bounds")]
fn a_third_enrichment_job_runs_for_launch_a_bounds(world: &OverpassEnrichmentWorld) {
    world.run_job();
}

#[when("one enrichment job fails for launch-a bounds")]
fn one_enrichment_job_fails_for_launch_a_bounds(world: &OverpassEnrichmentWorld) {
    world.run_job();
    if world.skip_if_needed() {
        return;
    }
    let result = world.last_result.get().expect("last result should exist");
    result.as_ref().expect_err("first job should fail");
}

#[when("the worker clock advances by 61 seconds")]
fn the_worker_clock_advances_by_61_seconds(world: &OverpassEnrichmentWorld) {
    world.advance_clock_seconds(61);
}

#[when("two enrichment jobs run concurrently for launch-a bounds")]
fn two_enrichment_jobs_run_concurrently_for_launch_a_bounds(world: &OverpassEnrichmentWorld) {
    if world.skip_if_needed() {
        return;
    }

    let runtime = world.runtime.get().expect("runtime should be set");
    let worker = world.worker.get().expect("worker should be set");
    let entered_rx = world
        .entered_rx
        .get()
        .expect("entered receiver should be set");
    let release_notify = world
        .release_notify
        .get()
        .expect("release notify should be set")
        .clone();

    let results = runtime.0.block_on(async {
        let first_worker = worker.clone();
        let first = tokio::spawn(async move {
            first_worker
                .process_job(OverpassEnrichmentRequest {
                    job_id: uuid::Uuid::new_v4(),
                    bounding_box: LAUNCH_A_BOUNDS,
                    tags: vec!["amenity".to_owned()],
                })
                .await
        });

        timeout(Duration::from_secs(1), async {
            entered_rx.lock().await.recv().await.expect("first entry")
        })
        .await
        .expect("first source call should enter");

        let second_worker = worker.clone();
        let second = tokio::spawn(async move {
            second_worker
                .process_job(OverpassEnrichmentRequest {
                    job_id: uuid::Uuid::new_v4(),
                    bounding_box: LAUNCH_A_BOUNDS,
                    tags: vec!["amenity".to_owned()],
                })
                .await
        });

        assert!(
            timeout(Duration::from_millis(80), async {
                entered_rx.lock().await.recv().await
            })
            .await
            .is_err(),
            "second call should wait on semaphore while first is active",
        );

        release_notify.notify_one();
        timeout(Duration::from_secs(1), async {
            entered_rx.lock().await.recv().await.expect("second entry")
        })
        .await
        .expect("second source call should enter after first releases");
        release_notify.notify_one();

        vec![
            first.await.expect("first join"),
            second.await.expect("second join"),
        ]
    });

    world.concurrent_results.set(results);
}

#[then("the worker reports a successful enrichment outcome")]
fn the_worker_reports_a_successful_enrichment_outcome(world: &OverpassEnrichmentWorld) {
    if world.skip_if_needed() {
        return;
    }

    let result = world.last_result.get().expect("last result should be set");
    let outcome = result.as_ref().expect("job should succeed");
    assert!(outcome.persisted_poi_count >= 1);
    assert!(outcome.transfer_bytes >= 1);
}

#[then("enrichment POIs are persisted")]
fn enrichment_pois_are_persisted(world: &OverpassEnrichmentWorld) {
    if world.skip_if_needed() {
        return;
    }
    let poi_count = world
        .query_poi_count()
        .expect("POI count should be available");
    assert!(poi_count >= 1);
}

#[then("an enrichment success metric is recorded")]
fn an_enrichment_success_metric_is_recorded(world: &OverpassEnrichmentWorld) {
    if world.skip_if_needed() {
        return;
    }

    let metrics = world.metrics.get().expect("metrics should be set");
    assert_eq!(metrics.success_count(), 1);
}

#[then("the worker fails with service unavailable")]
fn the_worker_fails_with_service_unavailable(world: &OverpassEnrichmentWorld) {
    if world.skip_if_needed() {
        return;
    }

    let result = world.last_result.get().expect("last result should be set");
    let error = result.as_ref().expect_err("job should fail");
    assert_eq!(error.code(), ErrorCode::ServiceUnavailable);
}

#[then("no Overpass source calls were made")]
fn no_overpass_source_calls_were_made(world: &OverpassEnrichmentWorld) {
    if world.skip_if_needed() {
        return;
    }
    let source = world.source.get().expect("source should be set");
    assert_eq!(source.call_count(), 0);
}

#[then("an enrichment quota failure metric is recorded")]
fn an_enrichment_quota_failure_metric_is_recorded(world: &OverpassEnrichmentWorld) {
    if world.skip_if_needed() {
        return;
    }

    let metrics = world.metrics.get().expect("metrics should be set");
    assert!(
        metrics
            .failure_kinds()
            .contains(&EnrichmentJobFailureKind::QuotaRequestLimit),
        "expected quota failure metric kind"
    );
}

#[then("the third job fails fast with service unavailable")]
fn the_third_job_fails_fast_with_service_unavailable(world: &OverpassEnrichmentWorld) {
    the_worker_fails_with_service_unavailable(world);
}

#[then("the source call count is two")]
fn the_source_call_count_is_two(world: &OverpassEnrichmentWorld) {
    if world.skip_if_needed() {
        return;
    }
    let source = world.source.get().expect("source should be set");
    assert_eq!(source.call_count(), 2);
}

#[then("an enrichment circuit-open metric is recorded")]
fn an_enrichment_circuit_open_metric_is_recorded(world: &OverpassEnrichmentWorld) {
    if world.skip_if_needed() {
        return;
    }

    let metrics = world.metrics.get().expect("metrics should be set");
    assert!(
        metrics
            .failure_kinds()
            .contains(&EnrichmentJobFailureKind::CircuitOpen),
        "expected circuit-open failure metric"
    );
}

#[then("an enrichment retry-exhausted metric is recorded")]
fn an_enrichment_retry_exhausted_metric_is_recorded(world: &OverpassEnrichmentWorld) {
    if world.skip_if_needed() {
        return;
    }
    let metrics = world.metrics.get().expect("metrics should be set");
    assert!(
        metrics
            .failure_kinds()
            .contains(&EnrichmentJobFailureKind::RetryExhausted),
        "expected retry-exhausted failure metric",
    );
}

#[then("both concurrent jobs complete successfully")]
fn both_concurrent_jobs_complete_successfully(world: &OverpassEnrichmentWorld) {
    if world.skip_if_needed() {
        return;
    }
    let results = world
        .concurrent_results
        .get()
        .expect("concurrent results should be set");
    assert_eq!(results.len(), 2);
    for result in results {
        result.as_ref().expect("concurrent job should succeed");
    }
}

#[then("the max observed concurrent source calls is one")]
fn the_max_observed_concurrent_source_calls_is_one(world: &OverpassEnrichmentWorld) {
    if world.skip_if_needed() {
        return;
    }
    let source = world.source.get().expect("source should be set");
    assert_eq!(source.max_active_count(), 1);
}
