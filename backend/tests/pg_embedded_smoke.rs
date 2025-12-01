//! Optional embedded Postgres smoke test gated by RUN_PG_EMBEDDED.
//! Use `cargo test -- --ignored` with `RUN_PG_EMBEDDED=1` to run it.

use pg_embedded_setup_unpriv::TestCluster;

/// Optional smoke test; enable with `RUN_PG_EMBEDDED=1`.
#[test]
#[ignore = "requires embedded Postgres binaries; opt-in via RUN_PG_EMBEDDED=1"]
fn pg_embedded_cluster_starts() {
    if std::env::var("RUN_PG_EMBEDDED").as_deref() != Ok("1") {
        eprintln!("SKIP-TEST-CLUSTER: set RUN_PG_EMBEDDED=1 to run");
        return;
    }

    let test_cluster = TestCluster::new().expect("embedded Postgres should start");
    let connection = test_cluster.connection();
    assert!(connection.port() > 0, "cluster exposes a port");
    let url = connection.database_url("app_db");
    assert!(
        url.starts_with("postgresql://"),
        "database URL should start with postgresql://"
    );
}
