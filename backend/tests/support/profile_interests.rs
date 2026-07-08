//! Shared profile and interests fixture values for integration tests.
//!
//! These flows exercise the same seeded profile values across integration and
//! behavioural suites, so keeping the common constants here avoids drift
//! between test crates.

pub const FIXTURE_AUTH_ID: &str = super::fixture_auth::FIXTURE_AUTH_ID;
pub const FIXTURE_PROFILE_NAME: &str = "Ada Lovelace";
pub const DB_PROFILE_NAME: &str = "Database Ada";
pub const FIRST_THEME_ID: &str = "3fa85f64-5717-4562-b3fc-2c963f66afa6";
pub const SECOND_THEME_ID: &str = "3fa85f64-5717-4562-b3fc-2c963f66afa7";
