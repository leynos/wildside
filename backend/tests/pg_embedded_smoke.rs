//! Optional embedded Postgres smoke test gated by RUN_PG_EMBEDDED.
//! Use `cargo test -- --ignored` with `RUN_PG_EMBEDDED=1` to run it.

use pg_embedded_setup_unpriv::TestCluster;

#[path = "support/test_env.rs"]
mod test_env;

/// Decide whether the opt-in smoke test should run.
///
/// The policy is separated from the read so it can be exercised with a stub
/// reader; `test_env::process_env` is the binary's only ambient lookup.
fn should_run_smoke_test(read_env: impl Fn(&str) -> Option<String>) -> bool {
    read_env("RUN_PG_EMBEDDED").as_deref() == Some("1")
}

/// Optional smoke test; enable with `RUN_PG_EMBEDDED=1`.
#[test]
#[ignore = "requires embedded Postgres binaries; opt-in via RUN_PG_EMBEDDED=1"]
fn pg_embedded_cluster_starts() {
    if !should_run_smoke_test(test_env::process_env) {
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

/// Scenario: `RUN_PG_EMBEDDED` is absent, set to `1`, or set to anything else.
///
/// Invariant: only the exact value `1` opts the smoke test in, and the
/// decision comes from the injected reader rather than the process.
#[test]
fn smoke_test_runs_only_when_explicitly_opted_in() {
    for (value, expected) in [(None, false), (Some("1"), true), (Some("0"), false)] {
        let should_run = should_run_smoke_test(|name| {
            (name == "RUN_PG_EMBEDDED")
                .then(|| value.map(str::to_owned))
                .flatten()
        });
        assert_eq!(
            should_run, expected,
            "RUN_PG_EMBEDDED={value:?} must decide the opt-in on its own"
        );
    }
}
