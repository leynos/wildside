//! Integration tests for template-cloned database isolation.

include!("support/entrypoint.rs");
declare_test_support!(atexit_cleanup, cluster_skip, embedded_postgres);

use postgres::{Client, NoTls};
use support::atexit_cleanup::{bootstrap_shared_cluster, ensure_stable_cluster_environment};
use support::cluster_skip::handle_cluster_setup_failure;
use support::embedded_postgres::provision_template_database;
use support::format_postgres_error;

#[test]
fn temporary_databases_are_isolated_from_template() {
    ensure_stable_cluster_environment()
        .expect("reconcile stable cluster environment before cluster access");
    let cluster = match bootstrap_shared_cluster() {
        Ok(cluster) => cluster,
        Err(reason) => {
            handle_cluster_setup_failure::<()>(reason);
            return;
        }
    };
    let db_one = match provision_template_database(cluster) {
        Ok(db) => db,
        Err(err) => {
            handle_cluster_setup_failure::<()>(err);
            return;
        }
    };
    let db_two = match provision_template_database(cluster) {
        Ok(db) => db,
        Err(err) => {
            handle_cluster_setup_failure::<()>(err);
            return;
        }
    };

    let mut client_one = match Client::connect(db_one.url(), NoTls) {
        Ok(client) => client,
        Err(err) => {
            panic!(
                "failed to connect to db one: {}",
                format_postgres_error(&err)
            );
        }
    };
    match client_one.batch_execute(concat!(
        "CREATE TABLE isolation_test (id INT PRIMARY KEY, value TEXT);",
        "INSERT INTO isolation_test (id, value) VALUES (1, 'alpha');"
    )) {
        Ok(()) => {}
        Err(err) => {
            panic!("failed to seed db one: {}", format_postgres_error(&err));
        }
    }

    let mut client_two = match Client::connect(db_two.url(), NoTls) {
        Ok(client) => client,
        Err(err) => {
            panic!(
                "failed to connect to db two: {}",
                format_postgres_error(&err)
            );
        }
    };
    let err = client_two
        .query("SELECT value FROM isolation_test", &[])
        .expect_err("db two should not see tables created in db one");
    let err_text = format_postgres_error(&err);
    assert!(
        err_text.contains("isolation_test"),
        "unexpected isolation error: {err_text}"
    );
}

/// The shared cluster accepts `UTC` on a fresh connection.
///
/// The wrapper's readiness check is what makes this meaningful: it only
/// returns a handle after the server has accepted the timezone, so later
/// suites can rely on UTC support without probing again.
#[test]
fn shared_cluster_accepts_the_utc_timezone() {
    ensure_stable_cluster_environment()
        .expect("reconcile stable cluster environment before cluster access");
    let cluster = match bootstrap_shared_cluster() {
        Ok(cluster) => cluster,
        Err(reason) => {
            handle_cluster_setup_failure::<()>(reason);
            return;
        }
    };

    let url = cluster.connection().database_url("postgres");
    let mut client = match Client::connect(&url, NoTls) {
        Ok(client) => client,
        Err(err) => {
            panic!(
                "failed to connect to the shared cluster: {}",
                format_postgres_error(&err)
            );
        }
    };
    client
        .batch_execute("SET TIME ZONE 'UTC'")
        .expect("shared cluster must accept the UTC timezone");
    let row = client
        .query_one("SHOW TimeZone", &[])
        .expect("read the session timezone");
    let timezone: String = row.get(0);
    assert_eq!(timezone, "UTC");
}
