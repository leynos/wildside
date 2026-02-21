//! Shared user-and-route seeding helpers for integration tests.

use backend::domain::UserId;
use postgres::{Client, NoTls};
use uuid::Uuid;

use super::format_postgres_error;

const DEFAULT_DISPLAY_NAME: &str = "Test User";

fn seed_user_and_route_with_display_name(
    client: &mut Client,
    user_id: &UserId,
    route_id: Uuid,
    display_name: &str,
) -> Result<(), String> {
    let user_uuid = *user_id.as_uuid();

    client
        .execute(
            "INSERT INTO users (id, display_name) VALUES ($1, $2)",
            &[&user_uuid, &display_name],
        )
        .map_err(|err| format_postgres_error(&err))?;

    client
        .execute(
            concat!(
                "INSERT INTO routes (id, user_id, path, generation_params) ",
                "VALUES ($1, $2, '((0,0),(1,1))'::path, '{}'::jsonb)"
            ),
            &[&route_id, &user_uuid],
        )
        .map_err(|err| format_postgres_error(&err))?;

    Ok(())
}

// Used by a subset of integration-test crates.
#[allow(dead_code)]
pub fn seed_user_and_route_with_client(
    client: &mut Client,
    user_id: &UserId,
    route_id: Uuid,
) -> Result<(), String> {
    seed_user_and_route_with_display_name(client, user_id, route_id, DEFAULT_DISPLAY_NAME)
}

// Used by a subset of integration-test crates.
#[allow(dead_code)]
pub fn seed_user_and_route(url: &str, user_id: &UserId, route_id: Uuid) -> Result<(), String> {
    let mut client = Client::connect(url, NoTls).map_err(|err| format_postgres_error(&err))?;
    seed_user_and_route_with_client(&mut client, user_id, route_id)
}
