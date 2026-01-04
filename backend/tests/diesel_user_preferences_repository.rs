//! Integration tests for `DieselUserPreferencesRepository` against embedded PostgreSQL.
//!
//! These tests validate the preferences repository contract using
//! `pg-embedded-setup-unpriv` for isolated PostgreSQL instances.

use backend::domain::ports::{UserPreferencesRepository, UserPreferencesRepositoryError};
use backend::domain::{UnitSystem, UserId, UserPreferences};
use backend::outbound::persistence::{DbPool, DieselUserPreferencesRepository, PoolConfig};
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

const TEST_DB: &str = "diesel_user_preferences_repo_test";

struct TestContext {
    runtime: Runtime,
    _cluster: TestCluster,
    repository: DieselUserPreferencesRepository,
    user_id: UserId,
}

fn seed_user(url: &str, user_id: &UserId) -> Result<(), String> {
    let mut client = Client::connect(url, NoTls).map_err(|err| format_postgres_error(&err))?;
    let display_name = "Preferences Test User";
    let user_uuid = *user_id.as_uuid();
    client
        .execute(
            "INSERT INTO users (id, display_name) VALUES ($1, $2)",
            &[&user_uuid, &display_name],
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
    seed_user(&database_url, &user_id)?;

    let config = PoolConfig::new(&database_url)
        .with_max_size(2)
        .with_min_idle(Some(1));
    let pool = runtime
        .block_on(async { DbPool::new(config).await })
        .map_err(|err| err.to_string())?;

    let repository = DieselUserPreferencesRepository::new(pool);

    Ok(TestContext {
        runtime,
        _cluster: cluster,
        repository,
        user_id,
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
fn preferences_repository_round_trip(repo_context: Option<TestContext>) {
    let Some(context) = repo_context else {
        eprintln!("SKIP-TEST-CLUSTER: preferences_repository_round_trip skipped");
        return;
    };

    let repository = context.repository.clone();
    let user_id = context.user_id.clone();
    let interest_id = Uuid::new_v4();
    let safety_id = Uuid::new_v4();
    let preferences = UserPreferences::builder(user_id.clone())
        .interest_theme_ids(vec![interest_id])
        .safety_toggle_ids(vec![safety_id])
        .unit_system(UnitSystem::Metric)
        .revision(1)
        .build();

    context
        .runtime
        .block_on(async { repository.save(&preferences, None).await })
        .expect("save preferences");

    let fetched = context
        .runtime
        .block_on(async { repository.find_by_user_id(&user_id).await })
        .expect("fetch preferences")
        .expect("preferences should exist");

    assert_eq!(fetched.user_id, user_id);
    assert_eq!(fetched.unit_system, UnitSystem::Metric);
    assert_eq!(fetched.revision, 1);
    assert_eq!(fetched.interest_theme_ids, vec![interest_id]);
    assert_eq!(fetched.safety_toggle_ids, vec![safety_id]);
}

#[rstest]
fn preferences_repository_updates_with_expected_revision(repo_context: Option<TestContext>) {
    let Some(context) = repo_context else {
        eprintln!(
            "SKIP-TEST-CLUSTER: preferences_repository_updates_with_expected_revision skipped"
        );
        return;
    };

    let repository = context.repository.clone();
    let user_id = context.user_id.clone();
    let initial = UserPreferences::builder(user_id.clone())
        .unit_system(UnitSystem::Metric)
        .revision(1)
        .build();

    context
        .runtime
        .block_on(async { repository.save(&initial, None).await })
        .expect("save initial preferences");

    let updated = UserPreferences::builder(user_id.clone())
        .unit_system(UnitSystem::Imperial)
        .revision(2)
        .build();

    context
        .runtime
        .block_on(async { repository.save(&updated, Some(1)).await })
        .expect("update preferences");

    let fetched = context
        .runtime
        .block_on(async { repository.find_by_user_id(&user_id).await })
        .expect("fetch preferences")
        .expect("preferences should exist");

    assert_eq!(fetched.unit_system, UnitSystem::Imperial);
    assert_eq!(fetched.revision, 2);
}

#[rstest]
fn preferences_repository_rejects_revision_mismatch(repo_context: Option<TestContext>) {
    let Some(context) = repo_context else {
        eprintln!("SKIP-TEST-CLUSTER: preferences_repository_rejects_revision_mismatch skipped");
        return;
    };

    let repository = context.repository.clone();
    let user_id = context.user_id.clone();
    let initial = UserPreferences::builder(user_id.clone())
        .unit_system(UnitSystem::Metric)
        .revision(2)
        .build();

    context
        .runtime
        .block_on(async { repository.save(&initial, None).await })
        .expect("save initial preferences");

    let updated = UserPreferences::builder(user_id.clone())
        .unit_system(UnitSystem::Imperial)
        .revision(3)
        .build();

    let error = context
        .runtime
        .block_on(async { repository.save(&updated, Some(1)).await })
        .expect_err("revision mismatch");

    assert!(matches!(
        error,
        UserPreferencesRepositoryError::RevisionMismatch { expected: 1, .. }
    ));
}
