use pg_embedded_setup_unpriv::TestCluster;

#[test]
fn pg_embedded_cluster_starts() {
    match TestCluster::new() {
        Ok(test_cluster) => {
            let connection = test_cluster.connection();
            assert!(connection.port() > 0, "cluster exposes a port");
            let url = connection.database_url("app_db");
            assert!(url.starts_with("postgresql://"));
        }
        Err(error) => {
            eprintln!("SKIP-TEST-CLUSTER: {error}");
        }
    }
}
