//! Connection-owning seed helpers for selected integration tests.
//!
//! These helpers open their own PostgreSQL connections to the shared embedded
//! cluster and delegate the actual row seeding to [`super::seed_helpers`]. This
//! keeps connection lifecycle management separate from the fixture-building
//! logic: callers that already hold a client use `seed_helpers` directly, while
//! callers that only need a one-shot seed use these connection-owning wrappers.

use backend::domain::UserId;
use postgres::{Client, NoTls};
use uuid::Uuid;

use super::format_postgres_error;
use super::seed_helpers::seed_user_and_route_with_client;

/// Seed a `users` row and matching `routes` row by creating a connection.
///
/// The following illustrates the expected call shape:
///
/// ```ignore
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
///
/// # Errors
///
/// Returns an error when the database connection cannot be established or when
/// seeding the `users` and `routes` rows fails.
pub fn seed_user_and_route(url: &str, user_id: &UserId, route_id: Uuid) -> Result<(), String> {
    let mut client = Client::connect(url, NoTls).map_err(|err| format_postgres_error(&err))?;
    seed_user_and_route_with_client(&mut client, user_id, route_id)
}
