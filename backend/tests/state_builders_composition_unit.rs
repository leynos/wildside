//! Unit tests proving fixture-mode adapter selection for HTTP state ports.
//!
//! These tests verify the fixture half of the composition invariant: when
//! `ServerConfig.db_pool` is `None`, every port resolves to a fixture adapter.
//! DB-mode composition is covered by the `startup_mode_composition_bdd` BDD
//! suite, which exercises all ports against embedded PostgreSQL.
//!
//! The tests use an observable-behaviour assertion strategy: each port is
//! exercised with a lightweight operation, and the response shape distinguishes
//! fixture from DB-backed implementations without requiring `TypeId`
//! introspection or changes to domain trait signatures.

use std::net::SocketAddr;
use std::sync::Arc;

use actix_web::cookie::Key;
use rstest::rstest;

mod support;

#[path = "../src/server/config.rs"]
mod server_config;
use server_config::ServerConfig;

#[path = "../src/server/state_builders.rs"]
mod state_builders;
use state_builders::build_http_state;

/// Helper to construct a fixture-mode `ServerConfig` with no database pool.
fn fixture_config() -> ServerConfig {
    let addr: SocketAddr = "127.0.0.1:8080".parse().expect("valid addr");
    let config = ServerConfig::new(
        Key::generate(),
        false,
        actix_web::cookie::SameSite::Lax,
        addr,
    );
    // Assert accessor round-trips — this also satisfies the dead-code lint in
    // this compilation unit, where mod.rs (containing create_server) is absent.
    assert_eq!(
        config.bind_addr(),
        addr,
        "bind_addr should round-trip the value supplied to new()"
    );
    #[cfg(feature = "metrics")]
    assert!(
        config.metrics().is_none(),
        "metrics should be None for a default-constructed ServerConfig"
    );
    config
}

/// Test that fixture mode builds a functional state and exhibits fixture behaviour.
///
/// This test exercises the login port as a representative smoke test. The key
/// assertion is that `admin`/`password` succeeds, which is the hallmark of the
/// `FixtureLoginService`. DB-backed login would reject these credentials.
#[rstest]
#[tokio::test]
async fn fixture_mode_wires_fixture_adapters() {
    use backend::domain::LoginCredentials;
    use backend::domain::ports::{FixtureRouteSubmissionService, RouteSubmissionService};

    let config = fixture_config();
    let route_submission: Arc<dyn RouteSubmissionService> = Arc::new(FixtureRouteSubmissionService);

    let state = build_http_state(&config, route_submission);

    // Fixture login accepts "admin"/"password" and returns a fixed user ID
    let fixture_creds =
        LoginCredentials::try_from_parts("admin", "password").expect("valid credentials");
    let login_result = state.login.authenticate(&fixture_creds).await;
    assert!(
        login_result.is_ok(),
        "fixture login should accept admin/password; got: {login_result:?}"
    );
    let user_id = login_result.expect("user id");
    assert_eq!(
        user_id.as_ref(),
        "123e4567-e89b-12d3-a456-426614174000",
        "fixture login returns fixed user ID"
    );

    // Users query succeeds (fixture returns static list)
    let users_result = state.users.list_users(&user_id).await;
    assert!(
        users_result.is_ok(),
        "fixture users query should succeed; got: {users_result:?}"
    );

    // Profile query returns a user for the fixture user ID
    let profile_result = state.profile.fetch_profile(&user_id).await;
    assert!(
        profile_result.is_ok(),
        "fixture profile query should succeed; got: {profile_result:?}"
    );

    // Preferences query returns default preferences
    let prefs_result = state.preferences_query.fetch_preferences(&user_id).await;
    assert!(
        prefs_result.is_ok(),
        "fixture preferences query should succeed; got: {prefs_result:?}"
    );

    // Catalogue explore returns a snapshot
    let catalogue_result = state.catalogue.explore_snapshot().await;
    assert!(
        catalogue_result.is_ok(),
        "fixture catalogue should succeed; got: {catalogue_result:?}"
    );

    // Enrichment provenance list returns empty records
    use backend::domain::ports::ListEnrichmentProvenanceRequest;
    let provenance_result = state
        .enrichment_provenance
        .list_recent(&ListEnrichmentProvenanceRequest {
            limit: 10,
            before: None,
        })
        .await;
    assert!(
        provenance_result.is_ok(),
        "fixture enrichment provenance should succeed; got: {provenance_result:?}"
    );
}

// DB-mode composition is tested by the `startup_mode_composition_bdd` BDD
// suite which exercises all ports against embedded PostgreSQL.
