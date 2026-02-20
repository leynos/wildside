//! Integration tests for `DieselWalkSessionRepository`.
//!
//! This suite validates walk-session persistence and completion-summary reads
//! against embedded PostgreSQL using the shared fixture pattern.

use backend::domain::ports::{WalkSessionRepository, WalkSessionRepositoryError};
use backend::domain::{
    UserId, WalkPrimaryStat, WalkPrimaryStatKind, WalkSecondaryStat, WalkSecondaryStatKind,
    WalkSession, WalkSessionDraft,
};
use backend::outbound::persistence::{DbPool, DieselWalkSessionRepository, PoolConfig};
use chrono::{Duration, Utc};
use pg_embedded_setup_unpriv::TemporaryDatabase;
use postgres::{Client, NoTls};
use rstest::{fixture, rstest};
use tokio::runtime::Runtime;
use uuid::Uuid;

mod support;

use support::atexit_cleanup::shared_cluster_handle;
use support::{format_postgres_error, handle_cluster_setup_failure, provision_template_database};

struct TestContext {
    runtime: Runtime,
    repository: DieselWalkSessionRepository,
    user_id: UserId,
    route_id: Uuid,
    database_url: String,
    _database: TemporaryDatabase,
}

fn seed_user_and_route(url: &str, user_id: &UserId, route_id: Uuid) -> Result<(), String> {
    let mut client = Client::connect(url, NoTls).map_err(|err| format_postgres_error(&err))?;
    let user_uuid = *user_id.as_uuid();
    let display_name = "Walk Session Test User";

    client
        .execute(
            "INSERT INTO users (id, display_name) VALUES ($1, $2)",
            &[&user_uuid, &display_name],
        )
        .map_err(|err| format_postgres_error(&err))?;

    client
        .execute(
            concat!(
                "INSERT INTO routes (id, user_id, path, generation_params) ",
                "VALUES ($1, $2, '((0,0),(1,1))'::path, '{}'::jsonb)"
            ),
            &[&route_id, &user_uuid],
        )
        .map_err(|err| format_postgres_error(&err))?;

    Ok(())
}

fn build_session(
    user_id: UserId,
    route_id: Uuid,
    started_at: chrono::DateTime<chrono::Utc>,
    ended_at: Option<chrono::DateTime<chrono::Utc>>,
) -> WalkSession {
    WalkSession::new(WalkSessionDraft {
        id: Uuid::new_v4(),
        user_id,
        route_id,
        started_at,
        ended_at,
        primary_stats: vec![
            WalkPrimaryStat::new(WalkPrimaryStatKind::Distance, 3650.0)
                .expect("valid distance stat"),
            WalkPrimaryStat::new(WalkPrimaryStatKind::Duration, 2820.0)
                .expect("valid duration stat"),
        ],
        secondary_stats: vec![
            WalkSecondaryStat::new(
                WalkSecondaryStatKind::Energy,
                320.0,
                Some("kcal".to_owned()),
            )
            .expect("valid energy stat"),
            WalkSecondaryStat::new(WalkSecondaryStatKind::Count, 12.0, None)
                .expect("valid count stat"),
        ],
        highlighted_poi_ids: vec![Uuid::new_v4()],
    })
    .expect("valid walk session")
}

fn drop_table(url: &str, table_name: &str) -> Result<(), String> {
    let mut client = Client::connect(url, NoTls).map_err(|err| format_postgres_error(&err))?;
    let escaped_name = table_name.replace('"', "\"\"");
    let sql = format!(r#"DROP TABLE IF EXISTS "{escaped_name}""#);
    client
        .batch_execute(sql.as_str())
        .map_err(|err| format_postgres_error(&err))
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
