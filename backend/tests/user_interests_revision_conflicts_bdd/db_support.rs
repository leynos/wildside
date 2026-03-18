//! DB bootstrap helpers for revision-safe interests BDD coverage.

use pg_embedded_setup_unpriv::TemporaryDatabase;
use postgres::{Client, NoTls};
use uuid::Uuid;

use backend::outbound::persistence::{DbPool, PoolConfig};

use super::super::support::atexit_cleanup::shared_cluster_handle;
use super::super::support::{format_postgres_error, provision_template_database};

pub(crate) struct DbContext {
    pub(crate) database_url: String,
    pub(crate) pool: DbPool,
    _database: TemporaryDatabase,
}

pub(crate) struct SeedPreferences<'a> {
    pub(crate) interest_ids: &'a [&'a str],
    pub(crate) safety_ids: &'a [&'a str],
    pub(crate) unit_system: &'a str,
    pub(crate) revision: i32,
}

#[derive(Default)]
pub(crate) struct World {
    pub(crate) db: Option<DbContext>,
    pub(crate) first_update: Option<super::Snapshot>,
    pub(crate) second_update: Option<super::Snapshot>,
    pub(crate) preferences: Option<super::Snapshot>,
    pub(crate) skip_reason: Option<String>,
}

pub(crate) fn is_skipped(world: &World) -> bool {
    if let Some(reason) = world.skip_reason.as_deref() {
        eprintln!("SKIP-TEST-CLUSTER: scenario skipped ({reason})");
        true
    } else {
        false
    }
}

pub(crate) fn setup_db_context() -> Result<DbContext, String> {
    let cluster = shared_cluster_handle().map_err(|error| error.to_string())?;
    let database = provision_template_database(cluster).map_err(|error| error.to_string())?;
    let database_url = database.url().to_owned();
    let pool = super::run_async(DbPool::new(
        PoolConfig::new(database_url.as_str())
            .with_max_size(2)
            .with_min_idle(Some(1)),
    ))
    .map_err(|error| error.to_string())?;
    Ok(DbContext {
        database_url,
        pool,
        _database: database,
    })
}

pub(crate) fn seed_user(url: &str, user_id: Uuid, display_name: &str) -> Result<(), String> {
    let mut client = Client::connect(url, NoTls).map_err(|error| format_postgres_error(&error))?;
    client
        .execute(
            "INSERT INTO users (id, display_name) VALUES ($1, $2)",
            &[&user_id, &display_name],
        )
        .map_err(|error| format_postgres_error(&error))
        .map(|_| ())
}

pub(crate) fn seed_preferences(
    url: &str,
    user_id: Uuid,
    prefs: SeedPreferences<'_>,
) -> Result<(), String> {
    let mut client = Client::connect(url, NoTls).map_err(|error| format_postgres_error(&error))?;
    let interest_ids = prefs
        .interest_ids
        .iter()
        .map(|value| Uuid::parse_str(value).expect("valid interest UUID"))
        .collect::<Vec<_>>();
    let safety_ids = prefs
        .safety_ids
        .iter()
        .map(|value| Uuid::parse_str(value).expect("valid safety UUID"))
        .collect::<Vec<_>>();
    client
        .execute(
            "INSERT INTO user_preferences (
                user_id,
                interest_theme_ids,
                safety_toggle_ids,
                unit_system,
                revision
            ) VALUES ($1, $2, $3, $4, $5)",
            &[
                &user_id,
                &interest_ids,
                &safety_ids,
                &prefs.unit_system,
                &prefs.revision,
            ],
        )
        .map_err(|error| format_postgres_error(&error))
        .map(|_| ())
}
