//! Persisted-boundary BDD coverage for domain job envelopes.

use std::sync::{Arc, Mutex};

use backend::domain::jobs::{BoundingBox, EnrichmentJob, EnrichmentJobParams, GenerateRouteJob};
use backend::domain::ports::{NoOpRouteQueueMetrics, RouteQueue, RouteSubmissionRequest};
use backend::domain::{IdempotencyKey, UserId};
use backend::outbound::queue::{ApalisPostgresProvider, GenericApalisRouteQueue, decode_job};
use chrono::{DateTime, Utc};
use pg_embedded_setup_unpriv::TemporaryDatabase;
use pretty_assertions::assert_eq;
use rstest::{fixture, rstest};
use rstest_bdd_macros::{given, scenario, then, when};
use serde_json::{Value, json};
use sqlx::PgPool;
use tokio::runtime::Runtime;
use uuid::Uuid;

include!("support/entrypoint.rs");
declare_test_support!(atexit_cleanup, embedded_postgres);

use support::atexit_cleanup::{ensure_stable_cluster_environment, shared_cluster_handle};
use support::embedded_postgres::provision_template_database;

const REQUEST_ID: Uuid = Uuid::from_bytes([0x61; 16]);
const ENRICHMENT_JOB_ID: Uuid = Uuid::from_bytes([0x62; 16]);

struct PersistedPayloads {
    route: Value,
    enrichment: Value,
}

struct JobPersistenceWorld {
    runtime: Runtime,
    pool: PgPool,
    route_job: GenerateRouteJob,
    enrichment_job: EnrichmentJob,
    enqueue_results: Vec<Result<(), String>>,
    persisted: Option<PersistedPayloads>,
    _database: TemporaryDatabase,
}

type SharedContext = Arc<Mutex<JobPersistenceWorld>>;

fn parse_timestamp(value: &'static str) -> DateTime<Utc> {
    match DateTime::parse_from_rfc3339(value) {
        Ok(timestamp) => timestamp.with_timezone(&Utc),
        Err(error) => panic!("static timestamp should parse: {error}"),
    }
}

fn route_job() -> GenerateRouteJob {
    let submission = RouteSubmissionRequest {
        idempotency_key: Some(IdempotencyKey::from_uuid(Uuid::from_bytes([0x63; 16]))),
        user_id: UserId::from_uuid(Uuid::from_bytes([0x64; 16])),
        payload: serde_json::from_value(json!({
            "origin": { "lat": 51.5074, "lng": -0.1278 },
            "destination": { "lat": 51.5014, "lng": -0.1419 },
            "preferences": { "mode": "walking" }
        }))
        .expect("route submission fixture should be valid"),
    };

    match GenerateRouteJob::try_from_submission(
        &submission,
        REQUEST_ID,
        parse_timestamp("2026-07-31T12:00:00Z"),
    ) {
        Ok(job) => job,
        Err(error) => panic!("static route job should be valid: {error}"),
    }
}

fn enrichment_job() -> EnrichmentJob {
    let bounding_box = match BoundingBox::new(-0.20, 51.40, 0.10, 51.60) {
        Ok(bounds) => bounds,
        Err(error) => panic!("static bounding box should be valid: {error}"),
    };
    let params = EnrichmentJobParams {
        job_id: ENRICHMENT_JOB_ID,
        idempotency_key: Some(IdempotencyKey::from_uuid(Uuid::from_bytes([0x65; 16]))),
        bounding_box,
        tags: vec!["tourism".to_owned(), "amenity".to_owned()],
        enqueued_at: parse_timestamp("2026-07-31T12:30:00Z"),
    };

    match EnrichmentJob::v1(params) {
        Ok(job) => job,
        Err(error) => panic!("static enrichment job should be valid: {error}"),
    }
}

fn setup_world() -> Result<JobPersistenceWorld, String> {
    ensure_stable_cluster_environment().map_err(|error| error.to_string())?;
    let runtime = Runtime::new().map_err(|error| error.to_string())?;
    let cluster = shared_cluster_handle().map_err(|error| error.to_string())?;
    let database = provision_template_database(cluster).map_err(|error| error.to_string())?;
    let pool = runtime
        .block_on(PgPool::connect(database.url()))
        .map_err(|error| error.to_string())?;

    Ok(JobPersistenceWorld {
        runtime,
        pool,
        route_job: route_job(),
        enrichment_job: enrichment_job(),
        enqueue_results: Vec::new(),
        persisted: None,
        _database: database,
    })
}

#[fixture]
fn world() -> SharedContext {
    match setup_world() {
        Ok(world) => Arc::new(Mutex::new(world)),
        Err(error) => panic!("job persistence setup failed: {error}"),
    }
}

#[given("valid generate-route and enrichment jobs")]
fn valid_generate_route_and_enrichment_jobs(world: &SharedContext) {
    let context = world.lock().expect("context lock");
    assert!(context.enqueue_results.is_empty());
    assert!(context.persisted.is_none());
}

#[when("I enqueue both jobs through Apalis PostgreSQL")]
fn enqueue_both_jobs_through_apalis_postgresql(world: &SharedContext) {
    let (handle, pool, route_job, enrichment_job) = {
        let context = world.lock().expect("context lock");
        (
            context.runtime.handle().clone(),
            context.pool.clone(),
            context.route_job.clone(),
            context.enrichment_job.clone(),
        )
    };

    let results = handle.block_on(async move {
        let provider = ApalisPostgresProvider::new(pool)
            .await
            .map_err(|error| error.to_string());
        match provider {
            Ok(provider) => {
                let route_queue: GenericApalisRouteQueue<GenerateRouteJob, _> =
                    GenericApalisRouteQueue::new(provider.clone(), Arc::new(NoOpRouteQueueMetrics));
                let enrichment_queue: GenericApalisRouteQueue<EnrichmentJob, _> =
                    GenericApalisRouteQueue::new(provider, Arc::new(NoOpRouteQueueMetrics));
                let route_result = route_queue.enqueue(&route_job).await;
                let enrichment_result = enrichment_queue.enqueue(&enrichment_job).await;
                vec![
                    route_result.map_err(|error| error.to_string()),
                    enrichment_result.map_err(|error| error.to_string()),
                ]
            }
            Err(error) => vec![Err(error)],
        }
    });

    world.lock().expect("context lock").enqueue_results = results;
}

async fn fetch_payload(pool: &PgPool, query: &str, value: Uuid) -> Value {
    let payload: String = sqlx::query_scalar(query)
        .bind(value.to_string())
        .fetch_one(pool)
        .await
        .expect("persisted job should be queryable");
    serde_json::from_str(&payload).expect("persisted job should contain JSON")
}

#[then("both JSON payloads are persisted in apalis.jobs")]
fn both_json_payloads_are_persisted(world: &SharedContext) {
    let (handle, pool, results) = {
        let context = world.lock().expect("context lock");
        (
            context.runtime.handle().clone(),
            context.pool.clone(),
            context.enqueue_results.clone(),
        )
    };
    assert_eq!(results.len(), 2, "both enqueue operations should run");
    assert!(
        results.iter().all(Result::is_ok),
        "both enqueue operations should succeed: {results:?}"
    );

    let persisted = handle.block_on(async {
        PersistedPayloads {
            route: fetch_payload(
                &pool,
                "SELECT convert_from(job, 'UTF8') FROM apalis.jobs \
                 WHERE convert_from(job, 'UTF8')::jsonb->>'requestId' = $1",
                REQUEST_ID,
            )
            .await,
            enrichment: fetch_payload(
                &pool,
                "SELECT convert_from(job, 'UTF8') FROM apalis.jobs \
                 WHERE convert_from(job, 'UTF8')::jsonb->>'jobId' = $1",
                ENRICHMENT_JOB_ID,
            )
            .await,
        }
    });
    assert_eq!(persisted.route["v"], "v1");
    assert_eq!(persisted.enrichment["v"], "v1");
    assert!(persisted.enrichment["boundingBox"].is_array());
    world.lock().expect("context lock").persisted = Some(persisted);
}

#[then("decode_job restores both typed job envelopes")]
fn decode_job_restores_both_typed_job_envelopes(world: &SharedContext) {
    let context = world.lock().expect("context lock");
    let persisted = context
        .persisted
        .as_ref()
        .expect("persisted payloads should be loaded");
    let route = decode_job::<GenerateRouteJob>(&persisted.route)
        .expect("persisted route job should decode");
    let enrichment = decode_job::<EnrichmentJob>(&persisted.enrichment)
        .expect("persisted enrichment job should decode");

    assert_eq!(route, context.route_job);
    assert_eq!(enrichment, context.enrichment_job);
}

#[scenario(path = "tests/features/job_structs_postgres.feature")]
#[rstest]
fn enqueue_and_decode_typed_jobs_at_the_postgresql_boundary(world: SharedContext) {
    assert!(
        world.lock().expect("context lock").persisted.is_some(),
        "scenario should load persisted payloads"
    );
}
