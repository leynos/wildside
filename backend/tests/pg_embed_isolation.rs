//! Integration tests for template-cloned database isolation.

#[path = "support/pg_embed.rs"]
mod pg_embed;

mod support;

use postgres::{Client, NoTls};
use support::{format_postgres_error, handle_cluster_setup_failure, provision_template_database};

#[test]
fn temporary_databases_are_isolated_from_template() {
    let cluster = match pg_embed::shared_cluster() {
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
