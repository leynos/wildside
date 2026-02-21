//! Integration tests for `DieselWalkSessionRepository`.
//!
//! This suite validates walk-session persistence and completion-summary reads
//! against embedded PostgreSQL using the shared fixture pattern.

use backend::domain::ports::{WalkSessionRepository, WalkSessionRepositoryError};
use backend::domain::{UserId, WalkSession};
use backend::outbound::persistence::{DbPool, DieselWalkSessionRepository, PoolConfig};
use chrono::{DateTime, Duration, Utc};
use pg_embedded_setup_unpriv::TemporaryDatabase;
use rstest::{fixture, rstest};
use tokio::runtime::Runtime;
use uuid::Uuid;

#[path = "diesel_walk_session_repository/test_params.rs"]
mod diesel_walk_session_repository_test_params;
mod support;

use crate::support::seed_helpers::seed_user_and_route;
use diesel_walk_session_repository_test_params::{WalkSessionStats, WalkSessionTestParams};
use support::atexit_cleanup::shared_cluster_handle;
use support::{drop_table, handle_cluster_setup_failure, provision_template_database};

struct TestContext {
    runtime: Runtime,
    repository: DieselWalkSessionRepository,
    user_id: UserId,
    route_id: Uuid,
    database_url: String,
    _database: TemporaryDatabase,
}

struct WalkSessionBuildSpec {
    started_at: DateTime<Utc>,
    ended_at: Option<DateTime<Utc>>,
    distance: f64,
    duration: f64,
    energy: f64,
    poi_count: f64,
}

fn build_session(
    user_id: UserId,
    route_id: Uuid,
    started_at: chrono::DateTime<chrono::Utc>,
    ended_at: Option<chrono::DateTime<chrono::Utc>>,
) -> WalkSession {
    let mut params = WalkSessionTestParams::new(user_id, route_id, started_at);
    if let Some(ended_at) = ended_at {
        params = params.with_ended_at(ended_at);
    }
    params.build()
}

fn build_session_with_id(
    id: Uuid,
    user_id: UserId,
    route_id: Uuid,
    spec: WalkSessionBuildSpec,
) -> WalkSession {
    let WalkSessionBuildSpec {
        started_at,
        ended_at,
        distance,
        duration,
        energy,
        poi_count,
    } = spec;

    let mut params = WalkSessionTestParams::new(user_id, route_id, started_at)
        .with_id(id)
        .with_stats(WalkSessionStats::new(distance, duration, energy, poi_count));
    if let Some(ended_at) = ended_at {
        params = params.with_ended_at(ended_at);
    }
    params.build()
}

fn setup_context() -> Result<TestContext, String> {
    let runtime = Runtime::new().map_err(|err| err.to_string())?;
    let cluster = shared_cluster_handle().map_err(|e| e.to_string())?;
    let temp_db = provision_template_database(cluster).map_err(|err| err.to_string())?;
    let database_url = temp_db.url().to_string();

    let user_id = UserId::random();
    let route_id = Uuid::new_v4();
    seed_user_and_route(database_url.as_str(), &user_id, route_id)?;

    let config = PoolConfig::new(database_url.as_str())
        .with_max_size(2)
        .with_min_idle(Some(1));
    let pool = runtime
        .block_on(async { DbPool::new(config).await })
        .map_err(|err| err.to_string())?;

    let repository = DieselWalkSessionRepository::new(pool);

    Ok(TestContext {
        runtime,
        repository,
        user_id,
        route_id,
        database_url,
        _database: temp_db,
    })
}

#[fixture]
fn repo_context() -> Option<TestContext> {
    match setup_context() {
        Ok(ctx) => Some(ctx),
        Err(reason) => handle_cluster_setup_failure(reason),
    }
}

#[rstest]
fn walk_repository_save_find_and_summary_filtering(repo_context: Option<TestContext>) {
    let Some(context) = repo_context else {
        eprintln!("SKIP-TEST-CLUSTER: walk_repository_save_find_and_summary_filtering skipped");
        return;
    };

    let repository = context.repository.clone();
    let now = Utc::now();
    let latest_completed = build_session(
        context.user_id.clone(),
        context.route_id,
        now,
        Some(now + Duration::minutes(47)),
    );
    let earlier_completed = build_session(
        context.user_id.clone(),
        context.route_id,
        now - Duration::hours(2),
        Some(now - Duration::hours(2) + Duration::minutes(30)),
    );
    let incomplete = build_session(
        context.user_id.clone(),
        context.route_id,
        now - Duration::hours(1),
        None,
    );

    context.runtime.block_on(async {
        repository
            .save(&latest_completed)
            .await
            .expect("save latest completed");
        repository
            .save(&earlier_completed)
            .await
            .expect("save earlier completed");
        repository.save(&incomplete).await.expect("save incomplete");
    });

    let found = context
        .runtime
        .block_on(async { repository.find_by_id(&latest_completed.id()).await })
        .expect("find saved session")
        .expect("saved session exists");
    assert_eq!(found.id(), latest_completed.id());

    let missing = context
        .runtime
        .block_on(async { repository.find_by_id(&Uuid::new_v4()).await })
        .expect("find missing session should succeed");
    assert!(missing.is_none(), "missing lookup should return none");

    let summaries = context
        .runtime
        .block_on(async {
            repository
                .list_completion_summaries_for_user(&context.user_id)
                .await
        })
        .expect("list completion summaries");

    assert_eq!(
        summaries.len(),
        2,
        "only completed sessions should be listed"
    );
    assert_eq!(summaries[0].session_id(), latest_completed.id());
    assert_eq!(summaries[1].session_id(), earlier_completed.id());
}

#[rstest]
fn walk_repository_upsert_persists_latest_values(repo_context: Option<TestContext>) {
    let Some(context) = repo_context else {
        eprintln!("SKIP-TEST-CLUSTER: walk_repository_upsert_persists_latest_values skipped");
        return;
    };

    let repository = context.repository.clone();
    let session_id = Uuid::new_v4();
    let started_at = Utc::now();
    let initial_session = build_session_with_id(
        session_id,
        context.user_id.clone(),
        context.route_id,
        WalkSessionBuildSpec {
            started_at,
            ended_at: None,
            distance: 1200.0,
            duration: 900.0,
            energy: 120.0,
            poi_count: 4.0,
        },
    );
    let updated_session = build_session_with_id(
        session_id,
        context.user_id.clone(),
        context.route_id,
        WalkSessionBuildSpec {
            started_at,
            ended_at: Some(started_at + Duration::minutes(30)),
            distance: 2500.0,
            duration: 1800.0,
            energy: 260.0,
            poi_count: 9.0,
        },
    );

    context.runtime.block_on(async {
        repository
            .save(&initial_session)
            .await
            .expect("initial save should succeed");
        repository
            .save(&updated_session)
            .await
            .expect("upsert save should succeed");
    });

    let found = context
        .runtime
        .block_on(async { repository.find_by_id(&session_id).await })
        .expect("find by id should succeed")
        .expect("session should exist after upsert");

    assert_eq!(found.id(), session_id);
    let found_ended_at = found
        .ended_at()
        .expect("upserted session should include completion timestamp");
    let expected_ended_at = updated_session
        .ended_at()
        .expect("updated session should include completion timestamp");
    let ended_at_delta_nanos = found_ended_at
        .signed_duration_since(expected_ended_at)
        .num_nanoseconds()
        .expect("timestamp difference should fit i64")
        .abs();
    assert!(
        ended_at_delta_nanos < 1_000,
        "ended_at should round-trip with microsecond precision; delta={ended_at_delta_nanos}ns"
    );
    assert_eq!(found.primary_stats(), updated_session.primary_stats());
    assert_eq!(found.secondary_stats(), updated_session.secondary_stats());
}

#[rstest]
fn walk_repository_summary_filters_out_other_users(repo_context: Option<TestContext>) {
    let Some(context) = repo_context else {
        eprintln!("SKIP-TEST-CLUSTER: walk_repository_summary_filters_out_other_users skipped");
        return;
    };

    let repository = context.repository.clone();
    let now = Utc::now();
    let user_a_session = build_session(
        context.user_id.clone(),
        context.route_id,
        now,
        Some(now + Duration::minutes(20)),
    );

    let other_user_id = UserId::random();
    let other_route_id = Uuid::new_v4();
    seed_user_and_route(
        context.database_url.as_str(),
        &other_user_id,
        other_route_id,
    )
    .expect("other user and route should seed");
    let user_b_session = build_session(
        other_user_id.clone(),
        other_route_id,
        now + Duration::minutes(5),
        Some(now + Duration::minutes(25)),
    );

    context.runtime.block_on(async {
        repository
            .save(&user_a_session)
            .await
            .expect("saving user A session should succeed");
        repository
            .save(&user_b_session)
            .await
            .expect("saving user B session should succeed");
    });

    let summaries = context
        .runtime
        .block_on(async {
            repository
                .list_completion_summaries_for_user(&context.user_id)
                .await
        })
        .expect("listing summaries for user A should succeed");
    let summary_ids: Vec<Uuid> = summaries
        .iter()
        .map(|summary| summary.session_id())
        .collect();

    assert!(
        summary_ids.contains(&user_a_session.id()),
        "user A's completed session should appear in their summaries"
    );
    assert!(
        !summary_ids.contains(&user_b_session.id()),
        "user B's session should not appear in user A summaries"
    );
}

#[rstest]
fn walk_repository_maps_missing_schema_to_query_error(repo_context: Option<TestContext>) {
    let Some(context) = repo_context else {
        eprintln!("SKIP-TEST-CLUSTER: walk_repository_maps_missing_schema_to_query_error skipped");
        return;
    };

    drop_table(context.database_url.as_str(), "walk_sessions").expect("drop table succeeds");

    let repository = context.repository.clone();
    let session = build_session(
        context.user_id.clone(),
        context.route_id,
        Utc::now(),
        Some(Utc::now() + Duration::minutes(12)),
    );
    let error = context
        .runtime
        .block_on(async { repository.save(&session).await })
        .expect_err("save should fail when table is missing");

    assert!(matches!(error, WalkSessionRepositoryError::Query { .. }));
}
