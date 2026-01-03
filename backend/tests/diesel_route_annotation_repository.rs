//! Integration tests for `DieselRouteAnnotationRepository` against embedded PostgreSQL.
//!
//! These tests validate note and progress persistence with revision checking
//! using `pg-embedded-setup-unpriv`.

use std::future::Future;
use std::pin::Pin;

use backend::domain::ports::{RouteAnnotationRepository, RouteAnnotationRepositoryError};
use backend::domain::{RouteNote, RouteNoteContent, RouteProgress, UserId};
use backend::outbound::persistence::{DbPool, DieselRouteAnnotationRepository, PoolConfig};
use pg_embedded_setup_unpriv::TestCluster;
use postgres::{Client, NoTls};
use rstest::{fixture, rstest};
use tokio::runtime::Runtime;
use uuid::Uuid;

#[path = "support/pg_embed.rs"]
mod pg_embed;

mod support;

use pg_embed::test_cluster;
use support::{
    format_postgres_error, handle_cluster_setup_failure, migrate_schema, reset_database,
};

const TEST_DB: &str = "diesel_route_annotation_repo_test";

struct TestContext {
    runtime: Runtime,
    _cluster: TestCluster,
    repository: DieselRouteAnnotationRepository,
    user_id: UserId,
    route_id: Uuid,
}

fn seed_user_and_route(url: &str, user_id: &UserId, route_id: Uuid) -> Result<(), String> {
    let mut client = Client::connect(url, NoTls).map_err(|err| format_postgres_error(&err))?;
    let display_name = "Annotation Test User";
    let user_uuid = *user_id.as_uuid();
    client
        .execute(
            "INSERT INTO users (id, display_name) VALUES ($1, $2)",
            &[&user_uuid, &display_name],
        )
        .map_err(|err| format_postgres_error(&err))?;

    let request_id = Uuid::new_v4();
    client
        .execute(
            concat!(
                "INSERT INTO routes (id, user_id, request_id, plan_snapshot) ",
                "VALUES ($1, $2, $3, '{}'::jsonb)"
            ),
            &[&route_id, &user_uuid, &request_id],
        )
        .map_err(|err| format_postgres_error(&err))?;

    Ok(())
}

fn setup_context() -> Result<TestContext, String> {
    let runtime = Runtime::new().map_err(|err| err.to_string())?;
    let cluster = test_cluster()?;
    reset_database(&cluster, TEST_DB).map_err(|err| err.to_string())?;
    let database_url = cluster.connection().database_url(TEST_DB);
    migrate_schema(&database_url).map_err(|err| err.to_string())?;

    let user_id = UserId::random();
    let route_id = Uuid::new_v4();
    seed_user_and_route(&database_url, &user_id, route_id)?;

    let config = PoolConfig::new(&database_url)
        .with_max_size(2)
        .with_min_idle(Some(1));
    let pool = runtime
        .block_on(async { DbPool::new(config).await })
        .map_err(|err| err.to_string())?;

    let repository = DieselRouteAnnotationRepository::new(pool);

    Ok(TestContext {
        runtime,
        _cluster: cluster,
        repository,
        user_id,
        route_id,
    })
}

#[fixture]
fn repo_context() -> Option<TestContext> {
    match setup_context() {
        Ok(ctx) => Some(ctx),
        Err(reason) => handle_cluster_setup_failure(reason),
    }
}

async fn assert_revision_mismatch_rejected<T, InitFn, UpdateFn, SaveFn>(
    repository: &DieselRouteAnnotationRepository,
    create_initial: InitFn,
    create_updated: UpdateFn,
    save: SaveFn,
) -> Result<(), RouteAnnotationRepositoryError>
where
    InitFn: FnOnce() -> T,
    UpdateFn: FnOnce() -> T,
    SaveFn: for<'a> Fn(
        &'a DieselRouteAnnotationRepository,
        T,
        Option<u32>,
    ) -> Pin<
        Box<dyn Future<Output = Result<(), RouteAnnotationRepositoryError>> + Send + 'a>,
    >,
{
    let initial = create_initial();
    save(repository, initial, None).await?;

    let updated = create_updated();
    let error = save(repository, updated, Some(2))
        .await
        .expect_err("revision mismatch");

    assert!(matches!(
        error,
        RouteAnnotationRepositoryError::RevisionMismatch { expected: 2, .. }
    ));

    Ok(())
}

#[rstest]
fn route_notes_round_trip(repo_context: Option<TestContext>) {
    let Some(context) = repo_context else {
        eprintln!("SKIP-TEST-CLUSTER: route_notes_round_trip skipped");
        return;
    };

    let repository = context.repository.clone();
    let note_id = Uuid::new_v4();
    let note = RouteNote::new(
        note_id,
        context.route_id,
        context.user_id.clone(),
        RouteNoteContent::new("First note"),
    );

    context
        .runtime
        .block_on(async { repository.save_note(&note, None).await })
        .expect("save note");

    let fetched = context
        .runtime
        .block_on(async { repository.find_note_by_id(&note_id).await })
        .expect("fetch note")
        .expect("note exists");

    assert_eq!(fetched.id, note_id);
    assert_eq!(fetched.route_id, context.route_id);
    assert_eq!(fetched.user_id, context.user_id);
    assert_eq!(fetched.body, "First note");
    assert_eq!(fetched.revision, 1);

    let notes = context
        .runtime
        .block_on(async {
            repository
                .find_notes_by_route_and_user(&context.route_id, &context.user_id)
                .await
        })
        .expect("list notes");
    assert_eq!(notes.len(), 1);
}

#[rstest]
fn route_notes_reject_revision_mismatch(repo_context: Option<TestContext>) {
    let Some(context) = repo_context else {
        eprintln!("SKIP-TEST-CLUSTER: route_notes_reject_revision_mismatch skipped");
        return;
    };

    let repository = context.repository.clone();
    let note_id = Uuid::new_v4();
    let route_id = context.route_id;
    let user_id = context.user_id.clone();

    context
        .runtime
        .block_on(async {
            assert_revision_mismatch_rejected(
                &repository,
                || {
                    RouteNote::new(
                        note_id,
                        route_id,
                        user_id.clone(),
                        RouteNoteContent::new("First note"),
                    )
                },
                || {
                    RouteNote::builder(note_id, route_id, user_id.clone())
                        .body("Updated note")
                        .revision(2)
                        .build()
                },
                |repo, note, expected| {
                    Box::pin(async move { repo.save_note(&note, expected).await })
                },
            )
            .await
        })
        .expect("revision mismatch test");
}

#[rstest]
fn route_notes_reject_unknown_route(repo_context: Option<TestContext>) {
    let Some(context) = repo_context else {
        eprintln!("SKIP-TEST-CLUSTER: route_notes_reject_unknown_route skipped");
        return;
    };

    let repository = context.repository.clone();
    let note_id = Uuid::new_v4();
    let missing_route = Uuid::new_v4();
    let note = RouteNote::new(
        note_id,
        missing_route,
        context.user_id.clone(),
        RouteNoteContent::new("Missing route note"),
    );

    let error = context
        .runtime
        .block_on(async { repository.save_note(&note, None).await })
        .expect_err("route not found");

    assert!(matches!(
        error,
        RouteAnnotationRepositoryError::RouteNotFound { .. }
    ));
}

#[rstest]
fn route_progress_round_trip(repo_context: Option<TestContext>) {
    let Some(context) = repo_context else {
        eprintln!("SKIP-TEST-CLUSTER: route_progress_round_trip skipped");
        return;
    };

    let repository = context.repository.clone();
    let stop_id = Uuid::new_v4();
    let progress = RouteProgress::builder(context.route_id, context.user_id.clone())
        .visited_stop_ids(vec![stop_id])
        .revision(1)
        .build();

    context
        .runtime
        .block_on(async { repository.save_progress(&progress, None).await })
        .expect("save progress");

    let fetched = context
        .runtime
        .block_on(async {
            repository
                .find_progress(&context.route_id, &context.user_id)
                .await
        })
        .expect("fetch progress")
        .expect("progress exists");

    assert_eq!(fetched.route_id, context.route_id);
    assert_eq!(fetched.user_id, context.user_id);
    assert_eq!(fetched.visited_stop_ids(), &[stop_id]);
    assert_eq!(fetched.revision, 1);
}

#[rstest]
fn route_progress_rejects_revision_mismatch(repo_context: Option<TestContext>) {
    let Some(context) = repo_context else {
        eprintln!("SKIP-TEST-CLUSTER: route_progress_rejects_revision_mismatch skipped");
        return;
    };

    let repository = context.repository.clone();
    let route_id = context.route_id;
    let user_id = context.user_id.clone();

    context
        .runtime
        .block_on(async {
            assert_revision_mismatch_rejected(
                &repository,
                || {
                    RouteProgress::builder(route_id, user_id.clone())
                        .visited_stop_ids(vec![Uuid::new_v4()])
                        .revision(1)
                        .build()
                },
                || {
                    RouteProgress::builder(route_id, user_id.clone())
                        .visited_stop_ids(vec![Uuid::new_v4()])
                        .revision(2)
                        .build()
                },
                |repo, progress, expected| {
                    Box::pin(async move { repo.save_progress(&progress, expected).await })
                },
            )
            .await
        })
        .expect("revision mismatch test");
}
