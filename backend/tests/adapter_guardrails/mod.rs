//! Integration guardrails for inbound adapters (HTTP + WebSocket).
//!
//! This integration test suite exercises real Actix handlers over real sockets
//! while substituting deterministic driving ports. It exists to keep inbound
//! adapters side effect free and ensure the domain remains framework-agnostic.

mod doubles;
mod harness;
mod steps;

#[path = "../support/ws.rs"]
mod ws_support;

use harness::{world, WorldFixture};
use rstest::rstest;

#[rstest]
fn http_happy_path_uses_injected_ports(world: WorldFixture) {
    let shared_world = world.world();

    steps::a_running_server_wired_with_mocked_http_and_ws_ports(shared_world.clone());
    steps::the_client_logs_in_with_valid_credentials(shared_world.clone());
    steps::the_http_response_is_success_and_a_session_cookie_is_set(shared_world.clone());
    steps::the_login_port_was_called_with_the_expected_credentials(shared_world.clone());

    steps::the_client_requests_the_users_list(shared_world.clone());
    steps::the_users_port_was_called_with_the_authenticated_user_id(shared_world.clone());
    steps::the_users_response_includes_the_expected_display_name(shared_world.clone());
}

#[rstest]
fn http_users_list_rejects_without_session(world: WorldFixture) {
    let shared_world = world.world();

    steps::a_running_server_wired_with_mocked_http_and_ws_ports(shared_world.clone());
    steps::the_client_requests_the_users_list_without_a_valid_session(shared_world.clone());
    steps::the_http_response_is_unauthorised(shared_world.clone());
    steps::the_users_port_is_not_called(shared_world.clone());
}

#[rstest]
fn http_unhappy_path_does_not_set_cookie(world: WorldFixture) {
    let shared_world = world.world();

    steps::a_running_server_wired_with_mocked_http_and_ws_ports(shared_world.clone());
    steps::the_client_logs_in_with_invalid_credentials(shared_world.clone());
    steps::the_http_response_is_unauthorised_and_no_session_cookie_is_set(shared_world.clone());

    {
        let ctx = shared_world.borrow();
        assert_eq!(
            ctx.login.calls(),
            vec![("admin".to_owned(), "wrong".to_owned())]
        );
        assert_eq!(ctx.users.calls(), Vec::<String>::new());
    }
}

#[rstest]
fn websocket_happy_path_uses_injected_port(world: WorldFixture) {
    let shared_world = world.world();

    steps::a_running_server_wired_with_mocked_http_and_ws_ports(shared_world.clone());
    steps::the_client_connects_to_the_websocket_and_submits_a_display_name(shared_world.clone());
    steps::the_websocket_response_is_a_user_created_event_and_the_port_is_called_once(
        shared_world.clone(),
    );

    steps::the_client_sends_malformed_json_over_the_websocket(shared_world.clone());
    steps::the_socket_closes_with_a_policy_error_and_the_port_is_not_called(shared_world.clone());
}

// -----------------------------------------------------------------------------
// Compilation guard (documents intent)
// -----------------------------------------------------------------------------

#[test]
fn domain_types_compile_in_test_context() {
    use backend::domain::{DisplayNameRejectedEvent, Error, ErrorCode, UserValidationError};
    use backend::TraceId;
    use uuid::Uuid;

    assert_eq!(Error::unauthorized("x").code(), ErrorCode::Unauthorized);
    let _ = doubles::UsersResponse::Err(Error::internal("boom"));
    let _ = DisplayNameRejectedEvent {
        trace_id: TraceId::from_uuid(Uuid::nil()),
        attempted_name: "bad".into(),
        error: UserValidationError::DisplayNameInvalidCharacters,
    };
}
