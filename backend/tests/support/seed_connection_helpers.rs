//! Connection-owning seed helpers for selected integration tests.

use backend::domain::UserId;
use postgres::{Client, NoTls};
use uuid::Uuid;

use super::format_postgres_error;
use super::seed_helpers::seed_user_and_route_with_client;

/// Seed a `users` row and matching `routes` row by creating a connection.
///
/// # Examples
///
/// ```no_run
/// use backend::domain::UserId;
/// use uuid::Uuid;
///
/// let user_id = UserId::random();
/// let route_id = Uuid::new_v4();
///
/// let result = crate::support::seed_connection_helpers::seed_user_and_route(
///     "postgres://localhost/test",
///     &user_id,
///     route_id,
/// );
/// assert!(result.is_ok());
/// ```
pub fn seed_user_and_route(url: &str, user_id: &UserId, route_id: Uuid) -> Result<(), String> {
    let mut client = Client::connect(url, NoTls).map_err(|err| format_postgres_error(&err))?;
    seed_user_and_route_with_client(&mut client, user_id, route_id)
}
